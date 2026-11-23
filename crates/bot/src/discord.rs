use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;
use poise::{BoxFuture, CreateReply, FrameworkError};
use tokio::sync::{OwnedRwLockReadGuard, RwLock, Semaphore};
use tokio::task;
use tracing::{debug, error, info, warn};

use foundry::common::architecture;
use foundry::domains::cat::animation as cat_animation;
use foundry::domains::cat::level;
use foundry::domains::enemy::animation as enemy_animation;
use foundry::domains::settings::lang;
use foundry::domains::stage::{Stage, sheet};
use foundry::systems::animation::MotionSet;
use foundry::systems::combat::registry::Magnification;

use super::whitelist;
use super::audio;
use super::backdrop;
use super::cache::Cache;
use super::config::Config;
use super::fuzzy;
use super::grammar::{self, Backing, Kind, Motion, Request, Scenery, Tune, Unit, Want};
use super::library::{self, Catalog, Library, Remote};
use super::lookup::{self, CROWN_GLYPH, Found, Hit};
use super::prefs::{self, Book, FORMS, Prefs, Search};
use super::prose::{self, Line, Prose, Trouble};
use super::render::{self, Format, Options, Planned, Progress};
use super::stage;
use super::statblock;
use super::talents;

const REFRESH: Duration = Duration::from_secs(60 * 60);
const TICK: Duration = Duration::from_secs(1);
const BAR: usize = 24;
const TRUE_FORM: usize = 2;
const LICENSES: &str = concat!(
    "========================================================================\nHAGANE\n========================================================================\n",
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE")),
    "\n",
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../foundry/assets/licenses.txt")),
);
const LEGAL: &str = "Not affiliated with PONOS Corp.";
const NOTICE_TRIES: usize = 5;
const TALENT_HINT: &str = "all, standard, ultra, none, show, 10 10 5 or Attack Buff 5";
const EMBED_COLOR: u32 = 0x2468A8;
const OPEN_SETTINGS: &str = "set|open";
const SET_PREFIX: &str = "set|prefix";
const FIND: &str = "find|";
const CROWN: &str = "crown|";
const THEME: &str = "t";
const BACKDROP: &str = "g";
const THEME_PRESS: &str = "theme|";
const SEEK: &str = "seek|";
const SEEK_JUMP: &str = "seekjump|";
const LIST_PAGES: usize = 99;
const LISTS_HELD: usize = 20_000;
const TOKEN_ROOM: u32 = 12;
const CHART: &str = "chart|";
const CHART_JUMP: &str = "chartjump|";
const JUMP: &str = "jump";
const BOSS: &str = "boss";
const STANDARD: &str = "standard";
const VALUE_LIMIT: usize = 100;
const OPTION_LIMIT: usize = 100;
const SEARCH_SHOWN: usize = 24;
const ABORT: &str = "abort";
const SET_JUMP: &str = "set|jump|";
const SETTINGS: [Setting; 6] = [Setting::Language, Setting::Format, Setting::Form, Setting::Search, Setting::Prose, Setting::Prefix];
const SETTINGS_ROWS: usize = 4;
const SETTINGS_PAGES: usize = SETTINGS.len().div_ceil(SETTINGS_ROWS);
const PAGE_DIGITS: u16 = 2;
const BLANK: &str = "-";
const LEVEL_LIMIT: u16 = 12;
const UPLOAD_LIMIT: usize = 20 * 1024 * 1024;
const MEGABYTE: f64 = 1024.0 * 1024.0;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
type Shelf = Arc<RwLock<Option<Arc<Catalog>>>>;
type Guard = OwnedRwLockReadGuard<Option<Arc<Catalog>>>;

pub(crate) struct Data {
    shelf: Shelf,
    cache: Arc<Cache>,
    sheets: Arc<Semaphore>,
    encoder: Arc<Semaphore>,
    book: Arc<Book>,
    lists: Mutex<Lists>,
}

type Choice = (String, String, String);

struct Lists {
    next: u64,
    held: VecDeque<(u64, u64, Arc<[Choice]>)>,
}

impl Lists {
    fn new(born: u64) -> Self {
        Self { next: born << TOKEN_ROOM, held: VecDeque::new() }
    }

    fn keep(&mut self, owner: u64, choices: Arc<[Choice]>) -> u64 {
        let token = self.next;

        self.next += 1;
        self.held.push_back((token, owner, choices));

        while self.held.len() > 1 && self.held.iter().map(|(_, _, choices)| choices.len()).sum::<usize>() > LISTS_HELD {
            self.held.pop_front();
        }

        token
    }

    fn find(&self, token: u64, owner: u64) -> Option<Arc<[Choice]>> {
        self.held.iter().find(|(held, asker, _)| *held == token && *asker == owner).map(|(_, _, choices)| Arc::clone(choices))
    }
}

fn lists(data: &Data) -> MutexGuard<'_, Lists> {
    data.lists.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) async fn run(config: Config, cache: Cache) -> Result<(), Error> {
    let shelf: Shelf = Arc::new(RwLock::new(None));
    let data = Data { shelf: Arc::clone(&shelf), cache: Arc::new(cache), sheets: Arc::new(Semaphore::new(1)), encoder: Arc::new(Semaphore::new(1)), book: Arc::new(Book::open()), lists: Mutex::new(Lists::new(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()))) };

    tokio::spawn(keep_fresh(config.source, shelf));

    let options = poise::FrameworkOptions {
        commands: vec![cat(), enemy(), stage(), map(), background(), audio(), settings(), website(), help(), invite(), legal(), donothing()],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: None,
            mention_as_prefix: false,
            stripped_dynamic_prefix: Some(invocation),
            case_insensitive_commands: true,
            ..Default::default()
        },
        command_check: Some(|ctx| {
            Box::pin(async move {
                let Some(notice) = refusal(ctx.guild_id(), Voice::Command(ctx).personal()) else {
                    return Ok(true);
                };

                say(ctx, notice.said(prose_of(ctx))).await?;
                Ok(false)
            })
        }),
        event_handler: |framework, event| Box::pin(on_event(framework.serenity_context, event, framework.user_data)),
        on_error: |failure| Box::pin(on_error(failure)),
        ..Default::default()
    };
    let framework = poise::Framework::builder()
        .options(options)
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                info!(user = ready.user.name, guilds = ready.guilds.len(), "Connected to Discord");
                if let Err(err) = poise::builtins::register_globally(ctx, &framework.options().commands).await {
                    error!("The slash commands could not be registered, the ones Discord already has stay in place: {}", err);
                }

                Ok(data)
            })
        })
        .build();
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::ClientBuilder::new(config.token, intents).framework(framework).await?;

    client.start().await?;

    Ok(())
}

fn refusal(guild: Option<serenity::GuildId>, personal: bool) -> Option<&'static Line> {
    guild.filter(|guild| !personal && !whitelist::admits(guild.get())).map(|_| &prose::UNLISTED)
}

fn encodes_for(personal: bool, user: serenity::UserId) -> bool {
    !personal || whitelist::trusts(user.get())
}

fn personal(guild: Option<serenity::GuildId>, owners: &serenity::AuthorizingIntegrationOwners) -> bool {
    guild.is_none() || !owners.0.iter().any(|owner| matches!(owner, serenity::AuthorizingIntegrationOwner::GuildInstall(_)))
}

fn prose_of(ctx: Context<'_>) -> Prose {
    ctx.data().book.prose(ctx.author().id.get())
}

fn invocation<'a>(ctx: &'a serenity::Context, msg: &'a serenity::Message, data: &'a Data) -> BoxFuture<'a, Result<Option<&'a str>, Error>> {
    Box::pin(async move {
        let chosen = data.book.prefix(msg.author.id.get());
        let prefix = chosen.as_deref().unwrap_or(prefs::PREFIX);
        let length = prefix_length(&msg.content, prefix, ctx.cache.current_user().id, msg.guild_id.is_none(), msg.referenced_message.as_ref().map(|reply| reply.author.id));

        Ok(length.and_then(|length| msg.content.get(..length)))
    })
}

fn prefix_length(content: &str, prefix: &str, me: serenity::UserId, direct: bool, replied_to: Option<serenity::UserId>) -> Option<usize> {
    let text = content.trim_start();
    let leading = content.len() - text.len();

    if text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)) {
        return Some(leading + prefix.len());
    }

    for mention in [format!("<@{me}>"), format!("<@!{me}>")] {
        if text.starts_with(mention.as_str()) {
            return Some(leading + mention.len());
        }
    }

    (direct || replied_to == Some(me)).then_some(leading)
}

async fn on_error(failure: FrameworkError<'_, Data, Error>) {
    match failure {
        FrameworkError::Command { error, ctx, .. } => {
            error!(command = %ctx.command().name, "A command failed: {}", error);

            if let Err(err) = say(ctx, prose::FAILED.said(prose_of(ctx))).await {
                error!("The failure notice could not be sent: {}", err);
            }
        }
        FrameworkError::UnknownCommand { msg, content_start, framework, .. } => {
            let (ctx, data) = (framework.serenity_context, framework.user_data);

            if let Some(notice) = refusal(msg.guild_id, msg.guild_id.is_none()) {
                if let Err(err) = msg.channel_id.say(ctx, notice.said(data.book.prose(msg.author.id.get()))).await {
                    warn!(guild = msg.guild_id.map(|guild| guild.get()), "The whitelist notice could not be sent: {}", err);
                }

                return;
            }

            let Some((prefix, text)) = msg.content.split_at_checked(usize::from(content_start)).filter(|(prefix, _)| !prefix.trim().is_empty()) else {
                return;
            };
            let voice = Voice::Message(ctx, msg, data);

            debug!(prefix, "A request came without a command");

            if let Err(err) = serve_any(voice, text).await {
                error!("A request without a command failed: {}", err);

                if let Err(err) = voice.say(prose::FAILED.said(voice.prose())).await {
                    error!("The failure notice could not be sent: {}", err);
                }
            }
        }
        FrameworkError::CommandCheckFailed { .. } => {}
        other => {
            if let Err(err) = poise::builtins::on_error(other).await {
                error!("An error could not be handled: {}", err);
            }
        }
    }
}

async fn keep_fresh(source: String, shelf: Shelf) {
    loop {
        let held = shelf.read().await.as_ref().map(|catalog| catalog.payload.clone());
        let link = source.clone();
        let checked = task::spawn_blocking(move || library::check(&link, held.as_deref())).await;

        match checked {
            Ok(Ok(Remote::Current)) => {}
            Ok(Ok(Remote::Changed(payload))) => {
                info!(payload, "Game data changed, updating");
                let mut slot = shelf.write().await;
                let link = source.clone();
                let loaded = task::spawn_blocking(move || {
                    library::download(&link).map(|payload| {
                        let catalog = Catalog::new(payload);

                        catalog.book(prefs::ENGLISH);
                        catalog
                    })
                })
                .await;

                match loaded {
                    Ok(Ok(catalog)) => *slot = Some(Arc::new(catalog)),
                    Ok(Err(reason)) => error!("The game data could not be updated: {}", reason),
                    Err(err) => error!("The game data update stopped: {}", err),
                }
            }
            Ok(Err(reason)) => {
                warn!("The game data source could not be checked: {}", reason);

                if shelf.read().await.is_none() && architecture::game_present() {
                    *shelf.write().await = Some(Arc::new(Catalog::new(String::new())));
                }
            }
            Err(err) => error!("The game data check stopped: {}", err),
        }

        tokio::time::sleep(REFRESH).await;
    }
}

async fn open(data: &Data, language: &str) -> Result<Option<(Guard, Arc<Library>)>, Error> {
    let guard = Arc::clone(&data.shelf).read_owned().await;
    let Some(catalog) = guard.as_ref().map(Arc::clone) else {
        return Ok(None);
    };
    let language = language.to_owned();
    let library = task::spawn_blocking(move || catalog.book(&language)).await?;

    Ok(Some((guard, library)))
}

async fn blocking<T: Send + 'static>(data: &Data, job: impl FnOnce() -> T + Send + 'static) -> Result<T, Error> {
    let _sheet = Arc::clone(&data.sheets).acquire_owned().await?;

    Ok(task::spawn_blocking(job).await?)
}

#[derive(Clone, Copy)]
enum Voice<'a> {
    Command(Context<'a>),
    Press(&'a serenity::Context, &'a serenity::ComponentInteraction, &'a Data),
    Message(&'a serenity::Context, &'a serenity::Message, &'a Data),
}

#[derive(Default)]
struct Post {
    text: String,
    files: Vec<(Vec<u8>, String)>,
    rows: Vec<serenity::CreateActionRow>,
}

impl Post {
    fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    fn file(bytes: Vec<u8>, name: String) -> Self {
        Self::files(vec![(bytes, name)])
    }

    fn files(files: Vec<(Vec<u8>, String)>) -> Self {
        Self { files, ..Self::default() }
    }

    fn rows(self, rows: Vec<serenity::CreateActionRow>) -> Self {
        Self { rows, ..self }
    }

    fn reply(self, clearing: bool) -> CreateReply {
        let reply = CreateReply::default().components(self.rows).allowed_mentions(quiet());
        let reply = if clearing || !self.text.is_empty() { reply.content(self.text) } else { reply };

        self.files.into_iter().fold(reply, |reply, (bytes, name)| reply.attachment(serenity::CreateAttachment::bytes(bytes, name)))
    }

    fn edit(self) -> serenity::EditInteractionResponse {
        let attachments = self.files.into_iter().fold(serenity::EditAttachments::new(), |held, (bytes, name)| held.add(serenity::CreateAttachment::bytes(bytes, name)));

        serenity::EditInteractionResponse::new().content(self.text).components(self.rows).attachments(attachments)
    }

    fn message(self) -> serenity::CreateMessage {
        let message = serenity::CreateMessage::new().components(self.rows).allowed_mentions(quiet());
        let message = if self.text.is_empty() { message } else { message.content(self.text) };

        self.files.into_iter().fold(message, |message, (bytes, name)| message.add_file(serenity::CreateAttachment::bytes(bytes, name)))
    }

    fn revision(self) -> serenity::EditMessage {
        let attachments = self.files.into_iter().fold(serenity::EditAttachments::new(), |held, (bytes, name)| held.add(serenity::CreateAttachment::bytes(bytes, name)));

        serenity::EditMessage::new().content(self.text).components(self.rows).attachments(attachments).allowed_mentions(quiet())
    }
}

enum Posted<'a> {
    Command(poise::ReplyHandle<'a>),
    Press,
    Message(serenity::MessageId),
}

impl<'a> Voice<'a> {
    fn data(self) -> &'a Data {
        match self {
            Voice::Command(ctx) => ctx.data(),
            Voice::Press(_, _, data) | Voice::Message(_, _, data) => data,
        }
    }

    fn user(self) -> serenity::UserId {
        match self {
            Voice::Command(ctx) => ctx.author().id,
            Voice::Press(_, press, _) => press.user.id,
            Voice::Message(_, msg, _) => msg.author.id,
        }
    }

    fn personal(self) -> bool {
        match self {
            Voice::Command(poise::Context::Application(ctx)) => personal(ctx.interaction.guild_id, &ctx.interaction.authorizing_integration_owners),
            Voice::Command(ctx) => ctx.guild_id().is_none(),
            Voice::Press(_, press, _) => personal(press.guild_id, &press.authorizing_integration_owners),
            Voice::Message(_, msg, _) => msg.guild_id.is_none(),
        }
    }

    fn prose(self) -> Prose {
        self.data().book.prose(self.user().get())
    }

    async fn working(self) -> Result<(), Error> {
        match self {
            Voice::Command(ctx @ poise::Context::Application(_)) => ctx.defer().await?,
            Voice::Command(ctx) => {
                if let Err(err) = ctx.http().broadcast_typing(ctx.channel_id()).await {
                    warn!("The typing indicator could not be shown: {}", err);
                }
            }
            Voice::Message(ctx, msg, _) => {
                if let Err(err) = msg.channel_id.broadcast_typing(ctx).await {
                    warn!("The typing indicator could not be shown: {}", err);
                }
            }
            Voice::Press(..) => {}
        }

        Ok(())
    }

    async fn post(self, post: Post) -> Result<Posted<'a>, Error> {
        match self {
            Voice::Command(ctx) => Ok(Posted::Command(send(ctx, post.reply(false)).await?)),
            Voice::Press(ctx, press, _) => {
                press.edit_response(ctx, post.edit()).await?;
                Ok(Posted::Press)
            }
            Voice::Message(ctx, msg, _) => {
                let message = post.message();
                let sent = match msg.channel_id.send_message(ctx, message.clone().reference_message(msg)).await {
                    Ok(sent) => sent,
                    Err(err) => {
                        warn!("An answer could not be sent as a reply, sending it on its own: {}", err);
                        msg.channel_id.send_message(ctx, message).await?
                    }
                };

                Ok(Posted::Message(sent.id))
            }
        }
    }

    async fn amend(self, posted: &Posted<'_>, post: Post) -> Result<(), Error> {
        match (self, posted) {
            (Voice::Command(ctx), Posted::Command(handle)) => handle.edit(ctx, post.reply(true)).await?,
            (Voice::Press(ctx, press, _), _) => {
                press.edit_response(ctx, post.edit()).await?;
            }
            (Voice::Message(ctx, msg, _), Posted::Message(sent)) => {
                msg.channel_id.edit_message(ctx, *sent, post.revision()).await?;
            }
            (Voice::Command(_), _) | (Voice::Message(..), _) => {}
        }

        Ok(())
    }

    async fn say(self, text: impl Into<String>) -> Result<(), Error> {
        self.post(Post::text(text)).await.map(|_| ())
    }
}

async fn offer(voice: Voice<'_>, prompt: String, labeled: bool, tails: &[(Kind, String)], found: Vec<Hit>) -> Result<(), Error> {
    offer_as(voice, prompt, labeled, None, tails, found).await
}

fn pager(shown: usize, pages: usize, turn: impl Fn(String) -> String) -> serenity::CreateActionRow {
    let button = |to: Option<usize>, side: &str, label: &str| serenity::CreateButton::new(turn(to.map_or_else(|| side.to_owned(), |to| to.to_string()))).label(label).style(serenity::ButtonStyle::Secondary).disabled(to.is_none());

    serenity::CreateActionRow::Buttons(vec![
        button(shown.checked_sub(1), "first", "◀"),
        serenity::CreateButton::new(turn(format!("{JUMP}{shown}"))).label(format!("{} / {pages}", shown + 1)).style(serenity::ButtonStyle::Secondary).disabled(pages <= 1),
        button(Some(shown + 1).filter(|next| *next < pages), "last", "▶"),
    ])
}

fn page_box(id: String, shown: usize, pages: usize) -> serenity::CreateInteractionResponse {
    let input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, format!("Page (1 to {pages})"), "page").placeholder((shown + 1).to_string()).max_length(PAGE_DIGITS).required(false);

    serenity::CreateInteractionResponse::Modal(serenity::CreateModal::new(id, "Go to page").components(vec![serenity::CreateActionRow::InputText(input)]))
}

fn list_pages(choices: &[Choice]) -> usize {
    choices.len().div_ceil(SEARCH_SHOWN).max(1)
}

fn list_rows(owner: u64, token: Option<u64>, choices: &[Choice], shown: usize) -> Vec<serenity::CreateActionRow> {
    let pages = list_pages(choices);
    let shown = shown.min(pages - 1);
    let options = choices
        .iter()
        .skip(shown * SEARCH_SHOWN)
        .take(SEARCH_SHOWN)
        .map(|(label, value, note)| serenity::CreateSelectMenuOption::new(label, value).description(note))
        .chain([serenity::CreateSelectMenuOption::new("Abort", ABORT).description("Stops the request")])
        .collect();
    let menu = serenity::CreateSelectMenu::new(format!("{FIND}{owner}"), serenity::CreateSelectMenuKind::String { options }).placeholder("Pick one");

    vec![serenity::CreateActionRow::SelectMenu(menu), pager(shown, pages, |slot| format!("{SEEK}{owner}|{:x}|{slot}", token.unwrap_or_default()))]
}

async fn offer_as(voice: Voice<'_>, prompt: String, labeled: bool, route: Option<&str>, tails: &[(Kind, String)], found: Vec<Hit>) -> Result<(), Error> {
    let owner = voice.user().get();
    let mut choices = Vec::with_capacity(found.len().min(SEARCH_SHOWN * LIST_PAGES));

    for hit in found.into_iter().take(SEARCH_SHOWN * LIST_PAGES) {
        let tail = tails.iter().find(|(kind, _)| *kind == hit.kind).map_or("", |(_, tail)| tail.as_str());
        let value = match (route, hit.kind) {
            (Some(route), Kind::Map) => format!("{}{route}|{}|{tail}", Kind::Map.tag(), hit.value),
            (Some(route), _) => format!("{route}|{}|{tail}", hit.value),
            (None, kind) => format!("{}|{}|{tail}", kind.tag(), hit.value),
        };

        if value.len() > VALUE_LIMIT {
            return voice.say(prose::SEARCH_LONG.said(voice.prose())).await;
        }

        let note = if labeled { format!("{} • {}", hit.kind.label(), hit.note) } else { hit.note };

        choices.push((hit.label.chars().take(OPTION_LIMIT).collect::<String>(), value, note.chars().take(OPTION_LIMIT).collect::<String>()));
    }

    let choices: Arc<[Choice]> = choices.into();
    let token = (choices.len() > SEARCH_SHOWN).then(|| lists(voice.data()).keep(owner, Arc::clone(&choices)));

    voice.post(Post::text(prompt).rows(list_rows(owner, token, &choices, 0))).await.map(|_| ())
}

async fn on_seek(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(3, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let token = parts.next().and_then(|token| u64::from_str_radix(token, 16).ok());
    let (Some(owner), Some(token), Some(slot)) = (owner, token, parts.next()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    let held = lists(data).find(token, owner);
    let Some(choices) = held else {
        let gone = serenity::CreateInteractionResponseMessage::new().content(prose::LIST_GONE.said(data.book.prose(owner))).ephemeral(true);

        press.create_response(ctx, serenity::CreateInteractionResponse::Message(gone)).await?;
        return Ok(());
    };

    if let Some(shown) = slot.strip_prefix(JUMP).and_then(|shown| shown.parse::<usize>().ok()) {
        press.create_response(ctx, page_box(format!("{SEEK_JUMP}{token:x}|{shown}"), shown, list_pages(&choices))).await?;
        return Ok(());
    }

    let Some(shown) = slot.parse::<usize>().ok() else { return Ok(()) };
    let turned = serenity::CreateInteractionResponseMessage::new().components(list_rows(owner, Some(token), &choices, shown));

    press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
    Ok(())
}

async fn serve(voice: Voice<'_>, kind: Kind, request: Request) -> Result<(), Error> {
    match kind {
        Kind::Cat => serve_cat(voice, request).await,
        Kind::Enemy => serve_enemy(voice, request).await,
        Kind::Stage => serve_stage(voice, request).await,
        Kind::Map => serve_map(voice, request).await,
    }
}

fn chart_pages(library: &Library, code: &str) -> Option<usize> {
    library.map(code).map(|area| library.map_stages(&area).len().div_ceil(SEARCH_SHOWN).max(1))
}

fn chart(owner: u64, library: &Library, code: &str, shown: usize, route: &str, tail: &str, prose: Prose) -> Result<Post, &'static Line> {
    let area = library.map(code).ok_or(&prose::NO_MAP)?;
    let stages = library.map_stages(&area);
    let pages = stages.len().div_ceil(SEARCH_SHOWN).max(1);
    let shown = shown.min(pages - 1);
    let named = area.kind().map_or_else(|| area.name().to_owned(), |kind| format!("{} • {kind}", area.name()));
    let turn = |slot: String| format!("{CHART}{owner}|{route}|{code}|{slot}|{tail}");
    let values: Vec<String> = stages.iter().map(|found| format!("{route}|{}|{tail}", found.code())).collect();

    if stages.is_empty() {
        return Ok(Post::text(prose::EMPTY_MAP.filled(prose, &[("map", &named)])));
    }

    if turn(format!("{JUMP}{pages}")).len() > VALUE_LIMIT || values.iter().any(|value| value.len() > VALUE_LIMIT) {
        return Err(&prose::SEARCH_LONG);
    }

    let options: Vec<serenity::CreateSelectMenuOption> = stages
        .iter()
        .zip(values)
        .skip(shown * SEARCH_SHOWN)
        .take(SEARCH_SHOWN)
        .map(|(found, value)| {
            let stage = found.code();
            let label = if found.name().trim().is_empty() { stage.as_str() } else { found.name() };
            let option = serenity::CreateSelectMenuOption::new(label.chars().take(OPTION_LIMIT).collect::<String>(), value);

            match lookup::stage_line(library, found).1 {
                Some(variant) => option.description(variant),
                None => option,
            }
        })
        .chain([serenity::CreateSelectMenuOption::new("Abort", ABORT).description("Stops the request")])
        .collect();
    let menu = serenity::CreateSelectMenu::new(format!("{FIND}{owner}"), serenity::CreateSelectMenuKind::String { options }).placeholder("Pick a stage");
    let rows = vec![serenity::CreateActionRow::SelectMenu(menu), pager(shown, pages, turn)];

    Ok(Post::text(prose::MAP_CHOOSE.filled(prose, &[("map", &named)])).rows(rows))
}

async fn listed(voice: Voice<'_>, library: &Library, code: &str, route: &str, tail: &str) -> Result<(), Error> {
    match chart(voice.user().get(), library, code, 0, route, tail, voice.prose()) {
        Ok(post) => voice.post(post).await.map(|_| ()),
        Err(line) => voice.say(line.said(voice.prose())).await,
    }
}

async fn charted(voice: Voice<'_>, data: &Data, owner: u64, code: &str, route: &str, tail: &str) -> Result<(), Error> {
    let Some((_guard, library)) = open(data, &data.book.get(owner).language).await? else {
        return voice.say(prose::WARMING.said(voice.prose())).await;
    };

    listed(voice, &library, code, route, tail).await
}

async fn serve_map(voice: Voice<'_>, request: Request) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let Some((_, code)) = sought(voice, &library, &prefs, &[Kind::Map], &request, None, &prose::NO_MAP).await? else { return Ok(()) };
    let tail = request.crowns.map_or_else(String::new, |crowns| format!("-c {crowns}"));

    listed(voice, &library, &code, Kind::Stage.tag(), &tail).await
}

async fn on_chart(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(5, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let (Some(owner), Some(route), Some(code), Some(slot), Some(tail)) = (owner, parts.next(), parts.next(), parts.next(), parts.next()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    let prefs = data.book.get(owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };

    if let Some(shown) = slot.strip_prefix(JUMP).and_then(|shown| shown.parse::<usize>().ok()) {
        press.create_response(ctx, page_box(format!("{CHART_JUMP}{route}|{code}|{shown}|{tail}"), shown, chart_pages(&library, code).unwrap_or(1))).await?;
        return Ok(());
    }

    let Some(shown) = slot.parse::<usize>().ok() else { return Ok(()) };
    let Ok(post) = chart(owner, &library, code, shown, route, tail, prefs.prose) else { return Ok(()) };
    let turned = serenity::CreateInteractionResponseMessage::new().content(post.text).components(post.rows);

    press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
    Ok(())
}

fn crown_rows(owner: u64, code: &str, stage: &Stage, shown: u8) -> Vec<serenity::CreateActionRow> {
    if stage.max_crowns <= 1 {
        return Vec::new();
    }

    let buttons = (0..stage.max_crowns)
        .map(|crown| {
            let open = stage::open(stage, crown);

            serenity::CreateButton::new(format!("{CROWN}{owner}|{code}|{crown}"))
                .label(format!("{}{CROWN_GLYPH}", crown + 1))
                .style(if crown == shown { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary })
                .disabled(!open)
        })
        .collect();

    vec![serenity::CreateActionRow::Buttons(buttons)]
}

async fn send_stage(voice: Voice<'_>, library: Arc<Library>, code: String, asked: Option<u8>) -> Result<(), Error> {
    let prose = voice.prose();
    let Some(found) = library.stage(&code) else {
        return voice.say(prose::NO_STAGE.said(prose)).await;
    };
    let crown = match stage::crown(found.stage, asked) {
        Ok(crown) => crown,
        Err(trouble) => return voice.say(trouble.told(prose)).await,
    };
    let rows = crown_rows(voice.user().get(), &code, found.stage, crown);
    let drawn = {
        let (library, cache, code) = (Arc::clone(&library), Arc::clone(&voice.data().cache), code.clone());

        blocking(voice.data(), move || stage::pages(&library, &cache, &code, crown)).await?
    };

    match drawn {
        Ok(pages) if pages.iter().map(|(bytes, _)| bytes.len()).sum::<usize>() > UPLOAD_LIMIT => voice.say(prose::STAGE_TOO_BIG.said(prose)).await,
        Ok(pages) => voice.post(Post::files(pages).rows(rows)).await.map(|_| ()),
        Err(trouble) => {
            if let Some(reason) = trouble.unexpected() {
                error!(code, "A stage could not be drawn: {}", reason);
            }

            voice.say(trouble.told(prose)).await
        }
    }
}

async fn sought(voice: Voice<'_>, library: &Library, prefs: &Prefs, kinds: &[Kind], request: &Request, route: Option<&str>, missing: &Line) -> Result<Option<(Kind, String)>, Error> {
    let asked = match &request.unit {
        Unit::Name(name) | Unit::Code(name) => name.clone(),
        Unit::Id(id, _) => id.to_string(),
    };
    let mut found = lookup::among(library, kinds, &request.unit, request.search.unwrap_or(fuzzy::CLOSENESS), prefs);
    let best = found.first().map(|hit| hit.score);

    if request.search.is_none() {
        found.retain(|hit| Some(hit.score) == best);
    }

    match (request.search.is_some(), found.as_slice()) {
        (_, []) => voice.say(missing.said(prefs.prose)).await.map(|()| None),
        (false, [only]) => Ok(Some((only.kind, only.value.clone()))),
        (searched, _) => {
            let line = if searched { &prose::SEARCH_PICK } else { &prose::EITHER_PICK };
            let tails: Vec<(Kind, String)> = kinds.iter().map(|kind| (*kind, request.tail.clone())).collect();

            offer_as(voice, line.filled(prefs.prose, &[("name", &asked)]), kinds.len() > 1, route, &tails, found).await.map(|()| None)
        }
    }
}

async fn serve_stage(voice: Voice<'_>, request: Request) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let Some((_, code)) = sought(voice, &library, &prefs, &[Kind::Stage], &request, None, &prose::NO_STAGE).await? else { return Ok(()) };

    send_stage(voice, Arc::clone(&library), code, request.crowns.map(|crowns| crowns.saturating_sub(1))).await
}

async fn on_crown(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(3, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let (Some(owner), Some(code), Some(crown)) = (owner, parts.next(), parts.next().and_then(|crown| crown.parse::<u8>().ok())) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    let voice = Voice::Press(ctx, press, data);

    press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    let Some((_guard, library)) = open(data, &data.book.get(owner).language).await? else {
        return Ok(());
    };

    send_stage(voice, library, code.to_owned(), Some(crown)).await
}

async fn serve_any(voice: Voice<'_>, text: &str) -> Result<(), Error> {
    let owner = voice.user().get();
    let prefs = voice.data().book.get(owner);
    let found = {
        let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
            return voice.say(prose::WARMING.said(prefs.prose)).await;
        };

        lookup::across(&library, &prefs, text)
    };

    match found {
        Found::Unread(reason) => voice.say(prose::SEE_HELP.filled(prefs.prose, &[("reason", &reason), ("prefix", &prefix_of(voice.data(), owner))])).await,
        Found::Nothing => voice.say(prose::NO_UNIT.filled(prefs.prose, &[("word", text.split_whitespace().next().unwrap_or_default()), ("prefix", &prefix_of(voice.data(), owner))])).await,
        Found::One(kind, request) => serve(voice, kind, request).await,
        Found::Several { asked, searched, tails, hits } => {
            let line = if searched { &prose::SEARCH_PICK } else { &prose::EITHER_PICK };

            offer(voice, line.filled(prefs.prose, &[("name", &asked)]), true, &tails, hits).await
        }
    }
}

async fn on_found(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let Ok(owner) = rest.parse::<u64>() else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    let serenity::ComponentInteractionDataKind::StringSelect { values } = &press.data.kind else { return Ok(()) };
    let Some(picked) = values.first() else { return Ok(()) };
    let voice = Voice::Press(ctx, press, data);

    press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    if picked == ABORT {
        return voice.say(prose::SEARCH_ABORT.said(voice.prose())).await;
    }

    let mut parts = picked.splitn(3, '|');
    let (Some(tag), Some(unit), Some(tail)) = (parts.next(), parts.next(), parts.next()) else { return Ok(()) };
    let kind = Kind::ALL.into_iter().find(|kind| kind.tag() == tag);
    let mapped = tag.strip_prefix(Kind::Map.tag()).filter(|route| [THEME, BACKDROP].contains(route));
    let served = match (tag, mapped, kind.map(|kind| (kind, grammar::parse(&format!("{unit} {tail}"), kind)))) {
        (_, Some(route), _) => charted(voice, data, owner, unit, route, tail).await,
        (THEME, ..) => themed(voice, data, owner, unit, tail == BOSS).await,
        (BACKDROP, ..) => backed(voice, data, owner, unit, tail).await,
        (_, _, Some((kind, Ok(request)))) => serve(voice, kind, request).await,
        (_, _, Some((_, Err(reason)))) => voice.say(prose::SEE_HELP.filled(voice.prose(), &[("reason", &reason), ("prefix", &prefix_of(data, owner))])).await,
        (_, _, None) => return Ok(()),
    };

    if let Err(err) = served {
        error!(picked, "A search pick failed: {}", err);
        voice.say(prose::FAILED.said(voice.prose())).await?;
    }

    Ok(())
}

fn quiet() -> serenity::CreateAllowedMentions {
    serenity::CreateAllowedMentions::new().replied_user(false)
}

async fn send<'a>(ctx: Context<'a>, reply: CreateReply) -> Result<poise::ReplyHandle<'a>, Error> {
    let reply = reply.allowed_mentions(quiet());

    if let poise::Context::Prefix(_) = ctx {
        match ctx.send(reply.clone().reply(true)).await {
            Ok(handle) => return Ok(handle),
            Err(err) => warn!("An answer could not be sent as a reply, sending it on its own: {}", err),
        }
    }

    Ok(ctx.send(reply).await?)
}

async fn say<'a>(ctx: Context<'a>, text: impl Into<String>) -> Result<poise::ReplyHandle<'a>, Error> {
    send(ctx, CreateReply::default().content(text)).await
}

fn file(bytes: Vec<u8>, name: String) -> CreateReply {
    CreateReply::default().attachment(serenity::CreateAttachment::bytes(bytes, name))
}

#[derive(Clone)]
struct Sheet {
    owner: u64,
    id: u32,
    form: usize,
    level: Option<String>,
    talents: Option<String>,
}

impl Sheet {
    fn custom(&self, tag: &str, form: usize) -> String {
        format!("{tag}|{}|{}|{}|{}|{}", self.owner, self.id, form, self.level.as_deref().unwrap_or_default(), self.talents.as_deref().unwrap_or("-"))
    }

    fn read(custom: &str) -> Option<(&str, Self)> {
        let mut parts = custom.split('|');
        let tag = parts.next()?;
        let owner = parts.next()?.parse().ok()?;
        let id = parts.next()?.parse().ok()?;
        let form = parts.next()?.parse().ok()?;
        let level = Some(parts.next()?).filter(|level| !level.is_empty()).map(str::to_owned);
        let talents = parts.next().filter(|levels| *levels != "-").map(str::to_owned);

        Some((tag, Self { owner, id, form, level, talents }))
    }
}

fn sheet_png(library: &Library, cache: &Cache, sheet: &Sheet) -> Result<(Vec<u8>, String), Trouble> {
    let cat = library.cat(sheet.id).ok_or(Trouble::NoCat)?;
    let caps = cat.unitbuy.level_caps();
    let levels = sheet.talents.as_deref().and_then(talents::decoded);
    let shown = sheet.level.as_deref().map_or_else(|| library.level_for(cat, sheet.form, levels.as_ref()), |level| level::capped(level, caps));
    let bytes = statblock::cat(library, cache, cat, sheet.form, level::total(&shown, caps), &shown, levels.as_ref())?;

    Ok((bytes, format!("{}.png", cat.id_str(sheet.form))))
}

fn sheet_rows(library: &Library, sheet: &Sheet) -> Vec<serenity::CreateActionRow> {
    let Some(cat) = library.cat(sheet.id) else {
        return Vec::new();
    };
    let forms: Vec<serenity::CreateButton> = (0..FORMS.len())
        .filter(|form| cat.forms[*form])
        .map(|form| {
            let current = form == sheet.form;

            serenity::CreateButton::new(sheet.custom("sb", form))
                .label(FORMS[form])
                .style(if current { serenity::ButtonStyle::Primary } else { serenity::ButtonStyle::Secondary })
                .disabled(current)
        })
        .collect();
    let mut rows = Vec::new();

    if forms.len() > 1 {
        rows.push(serenity::CreateActionRow::Buttons(forms));
    }

    let lit = |on: bool| if on { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary };
    let mut tuning = vec![serenity::CreateButton::new(sheet.custom("sl", sheet.form)).label("Level").style(lit(sheet.level.is_some()))];

    if cat.talent_data.is_some() && sheet.form >= TRUE_FORM {
        tuning.push(serenity::CreateButton::new(sheet.custom("st", sheet.form)).label("Talents").style(lit(sheet.talents.is_some())));
    }

    rows.push(serenity::CreateActionRow::Buttons(tuning));

    rows
}

async fn send_sheet(voice: Voice<'_>, library: Arc<Library>, sheet: Sheet) -> Result<(), Error> {
    let rendered = {
        let (library, sheet) = (Arc::clone(&library), sheet.clone());

        let cache = Arc::clone(&voice.data().cache);

        blocking(voice.data(), move || sheet_png(&library, &cache, &sheet)).await?
    };

    match rendered {
        Ok((bytes, name)) => voice.post(Post::file(bytes, name).rows(sheet_rows(&library, &sheet))).await.map(|_| ())?,
        Err(trouble) => voice.say(trouble.told(voice.prose())).await?,
    };

    Ok(())
}

fn clock(time: Duration) -> String {
    let seconds = time.as_secs();

    if seconds < 60 { format!("{seconds}s") } else { format!("{}m {:02}s", seconds / 60, seconds % 60) }
}

fn bar(share: f32) -> String {
    let filled = ((BAR as f32 * share.clamp(0.0, 1.0)) as usize).min(BAR);

    format!("`[{}{}]`", "#".repeat(filled), "-".repeat(BAR - filled))
}

fn percent(share: f32) -> String {
    format!("{}%", (share.clamp(0.0, 1.0) * 100.0) as u32)
}

fn progress_bar(title: &str, searching: bool, progress: &Progress, elapsed: Duration) -> String {
    let total = progress.total.load(Ordering::Relaxed);

    if searching && total == 0 {
        let share = elapsed.as_secs_f32() / render::LOOP_LIMIT.as_secs_f32();
        let searched = progress.searched.load(Ordering::Relaxed);
        let window = progress.window.load(Ordering::Relaxed);
        let count = if window > 0 { format!("{}/{window}", searched.min(window)) } else { searched.to_string() };

        return format!("{title}\nSearching • {} • {count}\n{}\nElapsed Time: {} • Estimated Time: under {}", percent(share), bar(share), clock(elapsed), clock(render::LOOP_LIMIT.saturating_sub(elapsed)));
    }

    let drawn = progress.drawn.load(Ordering::Relaxed).min(total);
    let done = progress.done.load(Ordering::Relaxed).min(total);
    let share = if total == 0 { 0.0 } else { (drawn + done) as f32 / (2 * total) as f32 };
    let estimate = if share > 0.0 { clock(Duration::from_secs_f32(elapsed.as_secs_f32() / share * (1.0 - share))) } else { "working it out".to_owned() };

    format!("{title}\nEncoding • {} • {done}/{total}\n{}\nElapsed Time: {} • Estimated Time: {estimate}", percent(share), bar(share), clock(elapsed))
}

async fn animate(voice: Voice<'_>, library: Arc<Library>, set: MotionSet, options: Options, title: String) -> Result<(), Error> {
    let (clip, format, options_looping) = (options.clip, options.format, options.looping.is_some());
    let prose = voice.prose();
    let cache = Arc::clone(&voice.data().cache);
    let planned = {
        let (library, cache) = (Arc::clone(&library), Arc::clone(&cache));

        task::spawn_blocking(move || render::plan(&library, &cache, &set, &options)).await?
    };
    let job = match planned {
        Ok(None) => {
            voice.say(prose::NO_CLIP.filled(prose, &[("unit", &title), ("clip", clip.motion())])).await?;
            return Ok(());
        }
        Ok(Some(Planned::Ready(bytes, _))) if bytes.len() > UPLOAD_LIMIT => {
            voice.say(oversized(bytes.len(), prose)).await?;
            return Ok(());
        }
        Ok(Some(Planned::Ready(bytes, name))) => {
            info!(title, clip = clip.slug(), format = format.extension(), name, "An animation came from the cache");
            voice.post(Post::file(bytes, name)).await?;
            return Ok(());
        }
        Ok(Some(Planned::Encode(job))) => job,
        Err(trouble) => {
            if let Some(reason) = trouble.unexpected() {
                error!(title, clip = clip.slug(), "An animation could not be prepared: {}", reason);
            }

            voice.say(trouble.told(prose)).await?;
            return Ok(());
        }
    };
    let label = format!("{title} • {} • {}", clip.motion(), format.label());

    encoded(voice, label, options_looping, move |cache, progress| job.run(cache, progress, UPLOAD_LIMIT as u64)).await
}

async fn encoded(voice: Voice<'_>, label: String, searching: bool, work: impl FnOnce(&Cache, &Progress) -> Result<(Vec<u8>, String), Trouble> + Send + 'static) -> Result<(), Error> {
    let prose = voice.prose();

    if !encodes_for(voice.personal(), voice.user()) {
        voice.say(prose::DM_ENCODE.said(prose)).await?;
        return Ok(());
    }

    let Ok(_encoding) = Arc::clone(&voice.data().encoder).try_acquire_owned() else {
        voice.say(prose::BUSY.said(prose)).await?;
        return Ok(());
    };
    let started = Instant::now();
    let progress = Arc::new(Progress::default());
    let posted = voice.post(Post::text(progress_bar(&label, searching, &progress, Duration::ZERO))).await?;
    let mut work = {
        let (cache, progress) = (Arc::clone(&voice.data().cache), Arc::clone(&progress));

        task::spawn_blocking(move || work(&cache, &progress))
    };
    let finished = loop {
        tokio::select! {
            done = &mut work => break done?,
            () = tokio::time::sleep(TICK) => {
                if let Err(err) = voice.amend(&posted, Post::text(progress_bar(&label, searching, &progress, started.elapsed()))).await {
                    warn!("The progress bar could not be updated: {}", err);
                }
            }
        }
    };

    match finished {
        Ok((bytes, _)) if bytes.len() > UPLOAD_LIMIT => voice.amend(&posted, Post::text(oversized(bytes.len(), prose))).await?,
        Ok((bytes, name)) => voice.amend(&posted, Post::file(bytes, name)).await?,
        Err(trouble) => {
            if let Some(reason) = trouble.unexpected() {
                error!(label, "An export could not be finished: {}", reason);
            }

            voice.amend(&posted, Post::text(trouble.told(prose))).await?;
        }
    }

    Ok(())
}

async fn send_background(voice: Voice<'_>, library: Arc<Library>, id: u32, scenery: &Scenery, prefs: &Prefs) -> Result<(), Error> {
    let options = backdrop::Options { id, seed: scenery.seed, range: scenery.range, offset: scenery.offset, size: scenery.size, format: scenery.format.unwrap_or(prefs.format), quality: scenery.quality, compression: scenery.compression };
    let format = options.format;
    let planned = {
        let (library, cache) = (Arc::clone(&library), Arc::clone(&voice.data().cache));

        task::spawn_blocking(move || backdrop::plan(&library, &cache, &options)).await?
    };

    match planned {
        Ok(backdrop::Planned::Ready(bytes, _)) if bytes.len() > UPLOAD_LIMIT => voice.say(oversized(bytes.len(), prefs.prose)).await,
        Ok(backdrop::Planned::Ready(bytes, name)) => {
            info!(id, format = format.extension(), name, "A background came from the cache");
            voice.post(Post::file(bytes, name)).await.map(|_| ())
        }
        Ok(backdrop::Planned::Encode(job)) => encoded(voice, format!("Background {id} • {}", format.label()), false, move |cache, progress| job.run(cache, progress, UPLOAD_LIMIT as u64)).await,
        Err(trouble) => {
            if let Some(reason) = trouble.unexpected() {
                error!(id, "A background could not be prepared: {}", reason);
            }

            voice.say(trouble.told(prefs.prose)).await
        }
    }
}

async fn serve_background(voice: Voice<'_>, scenery: Scenery) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let numbered = match &scenery.subject {
        Backing::Id(id, typed) if !backdrop::ids(&library).ids.contains(id) => grammar::place(typed).ok().map(|request| (request, &prose::NO_BACKGROUND)),
        _ => None,
    };
    let id = match (&scenery.subject, &numbered) {
        (Backing::Id(id, _), None) => *id,
        (Backing::Place(request), _) | (_, Some((request, _))) => {
            let missing = numbered.as_ref().map_or(&prose::NO_PLACE, |(_, missing)| missing);
            let Some((kind, code)) = sought(voice, &library, &prefs, &Kind::PLACES, request, Some(BACKDROP), missing).await? else { return Ok(()) };

            if kind == Kind::Map {
                return listed(voice, &library, &code, BACKDROP, &request.tail).await;
            }

            let Some(found) = library.stage(&code) else {
                return voice.say(prose::NO_STAGE.said(prefs.prose)).await;
            };

            found.stage.background_id
        }
    };

    send_background(voice, Arc::clone(&library), id, &scenery, &prefs).await
}

async fn backed(voice: Voice<'_>, data: &Data, owner: u64, code: &str, tail: &str) -> Result<(), Error> {
    let prefs = data.book.get(owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let Some(id) = library.stage(code).map(|found| found.stage.background_id) else {
        return voice.say(prose::NO_STAGE.said(prefs.prose)).await;
    };

    match grammar::scenery(&format!("{id} {tail}")) {
        Ok(scenery) => send_background(voice, Arc::clone(&library), id, &scenery, &prefs).await,
        Err(reason) => voice.say(prose::SEE_HELP.filled(prefs.prose, &[("reason", &reason), ("prefix", &prefix_of(data, owner))])).await,
    }
}

fn theme_rows(owner: u64, code: &str, boss: bool) -> Vec<serenity::CreateActionRow> {
    let button = |label: &str, slug: &str, shown: bool| serenity::CreateButton::new(format!("{THEME_PRESS}{owner}|{code}|{slug}")).label(label).style(if shown { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary });

    vec![serenity::CreateActionRow::Buttons(vec![button("Standard", STANDARD, !boss), button("Boss", BOSS, boss)])]
}

async fn send_theme(voice: Voice<'_>, library: Arc<Library>, code: String, boss: bool) -> Result<(), Error> {
    let prose = voice.prose();
    let Some(found) = library.stage(&code) else {
        return voice.say(prose::NO_STAGE.said(prose)).await;
    };
    let other = sheet::boss_track(found.stage);
    let id = match (boss, other) {
        (false, _) => found.stage.init_track,
        (true, Some(track)) => track,
        (true, None) => return voice.say(prose::NO_BOSS_THEME.filled(prose, &[("stage", found.name())])).await,
    };
    let (map, variant) = lookup::stage_line(&library, &found);
    let line: Vec<&str> = [Some(map).filter(|map| !map.is_empty()), Some(found.name()), variant].into_iter().flatten().collect();
    let text = format!("**{}**", line.join(" • "));
    let rows = if other.is_some() { theme_rows(voice.user().get(), &code, boss) } else { Vec::new() };
    let played = {
        let (library, cache) = (Arc::clone(&library), Arc::clone(&voice.data().cache));

        blocking(voice.data(), move || audio::track(&library, &cache, id)).await?
    };

    match played {
        Ok(None) => voice.say(prose::NO_AUDIO.said(prose)).await,
        Ok(Some(track)) if track.bytes.len() > UPLOAD_LIMIT => voice.say(oversized(track.bytes.len(), prose)).await,
        Ok(Some(track)) => voice.post(Post { text, files: vec![(track.bytes, track.name)], rows }).await.map(|_| ()),
        Err(trouble) => {
            if let Some(reason) = trouble.unexpected() {
                error!(code, id, "A stage's theme could not be sent: {}", reason);
            }

            voice.say(trouble.told(prose)).await
        }
    }
}

async fn themed(voice: Voice<'_>, data: &Data, owner: u64, code: &str, boss: bool) -> Result<(), Error> {
    let Some((_guard, library)) = open(data, &data.book.get(owner).language).await? else {
        return voice.say(prose::WARMING.said(voice.prose())).await;
    };

    send_theme(voice, library, code.to_owned(), boss).await
}

async fn serve_theme(voice: Voice<'_>, request: Request, boss: bool, missing: &Line) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let request = Request { tail: if boss { BOSS } else { STANDARD }.to_owned(), ..request };
    let Some((kind, code)) = sought(voice, &library, &prefs, &Kind::PLACES, &request, Some(THEME), missing).await? else { return Ok(()) };

    match kind {
        Kind::Map => listed(voice, &library, &code, THEME, &request.tail).await,
        _ => send_theme(voice, Arc::clone(&library), code, boss).await,
    }
}

async fn on_theme(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(3, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let (Some(owner), Some(code), Some(slug)) = (owner, parts.next(), parts.next()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;
    themed(Voice::Press(ctx, press, data), data, owner, code, slug == BOSS).await
}

async fn serve_audio(voice: Voice<'_>, id: u32, typed: String) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let found = {
        let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
            return voice.say(prose::WARMING.said(prefs.prose)).await;
        };
        let cache = Arc::clone(&voice.data().cache);

        blocking(voice.data(), move || audio::track(&library, &cache, id)).await?
    };

    match (found, grammar::place(&typed)) {
        (Ok(None), Ok(request)) => serve_theme(voice, request, false, &prose::NO_AUDIO).await,
        (Ok(None), Err(_)) => voice.say(prose::NO_AUDIO.said(prefs.prose)).await,
        (Ok(Some(track)), _) if track.bytes.len() > UPLOAD_LIMIT => voice.say(oversized(track.bytes.len(), prefs.prose)).await,
        (Ok(Some(track)), _) => {
            if !track.cached {
                info!(id, bytes = track.bytes.len(), "A sound effect was converted");
            }

            voice.post(Post::file(track.bytes, track.name)).await.map(|_| ())
        }
        (Err(trouble), _) => {
            if let Some(reason) = trouble.unexpected() {
                error!(id, "A track could not be sent: {}", reason);
            }

            voice.say(trouble.told(prefs.prose)).await
        }
    }
}

fn oversized(length: usize, prose: Prose) -> String {
    prose::OVERSIZED.filled(prose, &[("size", &format!("{:.1}", length as f64 / MEGABYTE))])
}

fn options_of(motion: &Motion, prefs: &Prefs) -> Options {
    Options {
        clip: motion.clip,
        format: motion.format.unwrap_or(prefs.format),
        range: motion.range,
        cull: motion.cull,
        overlays: motion.overlays,
        looping: motion.looping,
        background: motion.background,
        quality: motion.quality,
        compression: motion.compression,
    }
}

fn prefix_of(data: &Data, user: u64) -> String {
    data.book.prefix(user).unwrap_or_else(|| prefs::PREFIX.to_owned())
}

async fn understood(ctx: Context<'_>, request: Option<String>, kind: Kind) -> Result<Option<Request>, Error> {
    match grammar::parse(request.as_deref().unwrap_or_default(), kind) {
        Ok(request) => Ok(Some(request)),
        Err(reason) => {
            refused(ctx, &reason).await?;
            Ok(None)
        }
    }
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("c"))]
async fn cat(ctx: Context<'_>, #[description = "ID-form or name, then stats or anim"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Cat).await? else { return Ok(()) };

    serve_cat(Voice::Command(ctx), request).await
}

async fn serve_cat(voice: Voice<'_>, request: Request) -> Result<(), Error> {
    let owner = voice.user().get();
    let prefs = voice.data().book.get(owner);

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        voice.say(prose::WARMING.said(prefs.prose)).await?;
        return Ok(());
    };
    let request = lookup::settled(request, |name| library.names_cat(name));

    if let (Some(closeness), Unit::Name(name)) = (request.search, &request.unit) {
        let found = lookup::hits(&library, Kind::Cat, &request.unit, closeness, &prefs);

        if found.is_empty() {
            return voice.say(prose::NO_CAT.said(prefs.prose)).await;
        }

        return offer(voice, prose::SEARCH_PICK.filled(prefs.prose, &[("name", name)]), false, &[(Kind::Cat, request.tail)], found).await;
    }
    let found = match &request.unit {
        Unit::Id(id, form) => library.cat(*id).map(|cat| (cat, form.unwrap_or_else(|| prefs.form_from(&cat.forms)))),
        Unit::Name(name) => library.find_cat(name, &prefs),
        Unit::Code(_) => None,
    };
    let found = found.map(|(cat, form)| (cat, request.form.unwrap_or(form)));
    let Some((cat, form)) = found else {
        voice.say(prose::NO_CAT.said(prefs.prose)).await?;
        return Ok(());
    };

    if !cat.forms.get(form).copied().unwrap_or(false) {
        voice.say(prose::NO_FORM.filled(prefs.prose, &[("unit", &cat.display_name(0)), ("form", FORMS.get(form).copied().unwrap_or("such"))])).await?;
        return Ok(());
    }

    match request.want {
        Want::Stats(stats) if stats.talents.as_deref().is_some_and(talents::shows) => {
            let form = if form < TRUE_FORM && cat.forms[TRUE_FORM] { TRUE_FORM } else { form };

            voice.say(talents::overview(&library, cat, form, prefs.prose)).await?;
            Ok(())
        }
        Want::Stats(stats) => {
            let talents = match stats.talents {
                Some(words) => match talents::levels(&library, cat, &words) {
                    Ok(levels) => talents::encoded(&levels, cat.talent_data.as_ref().map_or(0, |data| data.groups.len())),
                    Err(trouble) => {
                        voice.say(trouble.told(prefs.prose)).await?;
                        return Ok(());
                    }
                },
                None => None,
            };
            let form = if talents.is_some() && form < TRUE_FORM && cat.forms[TRUE_FORM] { TRUE_FORM } else { form };
            let sheet = Sheet { owner, id: cat.id, form, level: stats.level, talents };

            send_sheet(voice, Arc::clone(&library), sheet).await
        }
        Want::Motion(motion) => {
            let set = cat_animation::motions(cat, form, &library.vault.vfs);
            let title = cat.display_name(form);

            animate(voice, Arc::clone(&library), set, options_of(&motion, &prefs), title).await
        }
    }
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("e"))]
async fn enemy(ctx: Context<'_>, #[description = "ID or name, then stats or anim"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Enemy).await? else { return Ok(()) };

    serve_enemy(Voice::Command(ctx), request).await
}

async fn serve_enemy(voice: Voice<'_>, request: Request) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        voice.say(prose::WARMING.said(prefs.prose)).await?;
        return Ok(());
    };
    let request = lookup::settled(request, |name| library.names_enemy(name));

    if let (Some(closeness), Unit::Name(name)) = (request.search, &request.unit) {
        let found = lookup::hits(&library, Kind::Enemy, &request.unit, closeness, &prefs);

        if found.is_empty() {
            return voice.say(prose::NO_ENEMY.said(prefs.prose)).await;
        }

        return offer(voice, prose::SEARCH_PICK.filled(prefs.prose, &[("name", name)]), false, &[(Kind::Enemy, request.tail)], found).await;
    }
    let found = match &request.unit {
        Unit::Id(id, _) => library.enemy(*id),
        Unit::Name(name) => library.find_enemy(name),
        Unit::Code(_) => None,
    };
    let Some(enemy) = found else {
        voice.say(prose::NO_ENEMY.said(prefs.prose)).await?;
        return Ok(());
    };

    match request.want {
        Want::Stats(stats) => {
            let (hitpoints, attack) = stats.magnification.unwrap_or_else(|| {
                let plain = Magnification::default();

                (plain.hitpoints, plain.attack)
            });
            let id = enemy.id;
            let rendered = {
                let (library, cache) = (Arc::clone(&library), Arc::clone(&voice.data().cache));

                blocking(voice.data(), move || {
                    let enemy = library.enemy(id).ok_or(Trouble::NoEnemy)?;
                    statblock::enemy(&library, &cache, enemy, Magnification { hitpoints, attack }).map(|bytes| (bytes, format!("{}.png", enemy.id_str())))
                })
                .await?
            };

            match rendered {
                Ok((bytes, name)) => voice.post(Post::file(bytes, name)).await.map(|_| ())?,
                Err(trouble) => voice.say(trouble.told(prefs.prose)).await?,
            };

            Ok(())
        }
        Want::Motion(motion) => {
            let set = enemy_animation::motions(enemy, &library.vault.vfs);
            let title = enemy.display_name();

            animate(voice, Arc::clone(&library), set, options_of(&motion, &prefs), title).await
        }
    }
}

async fn refused(ctx: Context<'_>, reason: &str) -> Result<(), Error> {
    say(ctx, prose::SEE_HELP.filled(prose_of(ctx), &[("reason", reason), ("prefix", &prefix_of(ctx.data(), ctx.author().id.get()))])).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("s"))]
async fn stage(ctx: Context<'_>, #[description = "A stage's name or ID like R 1 2, then --crowns 1 to 4"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Stage).await? else { return Ok(()) };

    serve_stage(Voice::Command(ctx), request).await
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("m"))]
async fn map(ctx: Context<'_>, #[description = "A map's name or ID like R 100, then --crowns 1 to 4"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Map).await? else { return Ok(()) };

    serve_map(Voice::Command(ctx), request).await
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("bg", "b"))]
async fn background(ctx: Context<'_>, #[description = "ID or stage name, then --seed, --range, --offset, --dimensions or --format"] #[rest] request: Option<String>) -> Result<(), Error> {
    match grammar::scenery(request.as_deref().unwrap_or_default()) {
        Ok(scenery) => serve_background(Voice::Command(ctx), scenery).await,
        Err(reason) => refused(ctx, &reason).await,
    }
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("a"))]
async fn audio(ctx: Context<'_>, #[description = "A track's ID like 5, or a stage's name, then --boss"] #[rest] id: Option<String>) -> Result<(), Error> {
    match grammar::tune(id.as_deref().unwrap_or_default()) {
        Ok(Tune::Track(id, typed)) => serve_audio(Voice::Command(ctx), id, typed).await,
        Ok(Tune::Place(request, boss)) => serve_theme(Voice::Command(ctx), request, boss, &prose::NO_PLACE).await,
        Err(reason) => refused(ctx, &reason).await,
    }
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("h"))]
async fn help(ctx: Context<'_>, #[description = "A command to explain, like cat or enemy"] #[rest] command: Option<String>) -> Result<(), Error> {
    let asked = command.as_deref().map(str::trim).filter(|asked| !asked.is_empty());
    let owner = ctx.author().id.get();
    let prefix = prefix_of(ctx.data(), owner);
    let lowered = asked.map(str::to_lowercase).unwrap_or_default();
    let mut words = lowered.split_whitespace();
    let named = words.next().map(|word| word.strip_prefix(prefix.to_lowercase().as_str()).or_else(|| word.strip_prefix(prefs::PREFIX)).unwrap_or(word));
    let page = named.map_or(Some(HOME), canonical);

    match page {
        Some(page) => {
            let section = section_of(page, words.next().unwrap_or_default());

            send(ctx, CreateReply::default().embed(help_page(page, section, &prefix, prose_of(ctx))).components(help_rows(owner, page, section))).await?
        }
        None => say(ctx, prose::NO_PAGE.filled(prose_of(ctx), &[("name", asked.unwrap_or_default()), ("prefix", &prefix)])).await?,
    };

    Ok(())
}

const HOME: &str = "home";
const PAGES: [(&str, &str); 12] = [(HOME, "Home"), ("cat", "Cat"), ("enemy", "Enemy"), ("stage", "Stage"), ("map", "Map"), ("background", "Background"), ("audio", "Audio"), ("settings", "Settings"), ("website", "Website"), ("invite", "Invite"), ("legal", "Legal"), ("help", "Help")];
const PAGES_PER_ROW: usize = 5;
const GENERAL: &str = "general";
const UNIT_SECTIONS: [(&str, &str); 4] = [(GENERAL, "General"), ("unit", "Unit"), ("stats", "Stats"), ("animation", "Animation")];
const CAT_SECTIONS: [(&str, &str); 5] = [(GENERAL, "General"), ("unit", "Unit"), ("stats", "Stats"), ("animation", "Animation"), ("buttons", "Buttons")];

fn sections(page: &str) -> &'static [(&'static str, &'static str)] {
    match page {
        "cat" => &CAT_SECTIONS,
        "enemy" => &UNIT_SECTIONS,
        _ => &[],
    }
}

fn section_of(page: &str, asked: &str) -> &'static str {
    let held = sections(page);

    held.iter().find(|(key, _)| *key == asked || key.starts_with(asked) && !asked.is_empty()).or(held.first()).map_or("", |(key, _)| key)
}

fn canonical(asked: &str) -> Option<&'static str> {
    match asked {
        "home" | "general" => Some(HOME),
        "cat" | "c" => Some("cat"),
        "enemy" | "e" => Some("enemy"),
        "stage" | "s" => Some("stage"),
        "map" | "m" => Some("map"),
        "background" | "bg" | "b" => Some("background"),
        "audio" | "a" => Some("audio"),
        "settings" | "set" => Some("settings"),
        "website" | "web" | "w" => Some("website"),
        "invite" | "i" | "inv" => Some("invite"),
        "legal" | "l" | "license" => Some("legal"),
        "help" | "h" => Some("help"),
        _ => None,
    }
}

fn help_rows(owner: u64, current: &str, section: &str) -> Vec<serenity::CreateActionRow> {
    let button = |custom: String, label: &str, here: bool| serenity::CreateButton::new(custom).label(label).style(if here { serenity::ButtonStyle::Primary } else { serenity::ButtonStyle::Secondary }).disabled(here);
    let held = sections(current);
    let buttons: Vec<serenity::CreateButton> = if held.is_empty() {
        PAGES.iter().map(|(page, label)| button(format!("help|{owner}|{page}"), label, *page == current)).collect()
    } else {
        std::iter::once(button(format!("help|{owner}|{HOME}"), "Home", false)).chain(held.iter().map(|(key, label)| button(format!("help|{owner}|{current}|{key}"), label, *key == section))).collect()
    };
    let rows = buttons.len().div_ceil(PAGES_PER_ROW).max(1);

    buttons.chunks(buttons.len().div_ceil(rows).max(1)).map(|row| serenity::CreateActionRow::Buttons(row.to_vec())).collect()
}

fn page(title: &str, description: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new().title(title).description(description).color(EMBED_COLOR)
}

const CLIPS: &str = "`attack` (`atk`, `a`), `idle` (`i`), `walk` (`w`), `knockback` (`kb`, `k`), `showcase` (`sh`, `sc`), `burrow` (`bur`, `b`), `surface` (`su`, `sf`, `rise`, `r`), `dig` (`d`), `spirit` (`sp`, `c`, `conjure`), `model` (`m`) for a single picture of the unit with no animation. Attack if none is given.";
const MOTION_FLAGS: &str = "`--range 0~30` or `--range 0 30` (`-r`): frames to export, wrapping around the loop; not for showcases or models
`--loop` (`-l`), walk and idle only: finds a perfect loop; `min 15`, `max 200` and `diff 30` tune it, and with `--range` it only searches that window
`--format webp` (`-f`): `avif` (`a`), `webp` (`w`) or `gif` (`g`), this time only, over your default
`--background` (`-b`): a solid gray background instead of a transparent one
`--quality 90` (`-q`): 0 to 100, 80 to start
`--compress 30` (`-p`): 0 to 100, 0 to start; higher takes longer
`--cull 40` (`-u`): bounds cull %, for long beams and faint effects; spirits are never culled
`--combat` (`-c`), `--rig` (`-g`), `--world` (`-w`), `--origin` (`-o`): debug overlays";

const DEFAULT_PREFIX: &str = "{default}";

fn voiced(text: &str, prefix: &str) -> String {
    text.replace(prefs::PREFIX, prefix).replace(DEFAULT_PREFIX, prefs::PREFIX)
}

struct Voiced<'a> {
    prefix: &'a str,
    embed: serenity::CreateEmbed,
}

impl<'a> Voiced<'a> {
    fn new(prefix: &'a str, title: &str, description: &str) -> Self {
        Self { prefix, embed: page(&voiced(title, prefix), &voiced(description, prefix)) }
    }

    fn field(self, name: &str, text: &str, inline: bool) -> Self {
        Self { prefix: self.prefix, embed: self.embed.field(voiced(name, self.prefix), voiced(text, self.prefix), inline) }
    }
}

const NAMES: &str = "Spaces are fine, and close spellings still match";
const SEARCHING: &str = "After a name: lists the closest matches for you to pick from, 24 to a page. ◀ and ▶ under the list turn its pages, and the page number between them opens a box to jump to one. A number after it, 1 to 100, is how close a name has to be: 86 to start, lower finds more";

fn help_page(page_name: &str, section: &str, prefix: &str, prose: Prose) -> serenity::CreateEmbed {
    let voiced = match (page_name, section) {
        ("cat", "unit") => Voiced::new(prefix, "hg!cat (c) • Unit", "What goes where `<unit>` is in `hg!cat <unit> ...`")
            .field("ID", "`25-3`, `25_t` or `25_s`: an ID and a form (1-4, n e t u, or f c s u)\n`25` or `025`: your default form, or the next one down if it has none", false)
            .field("Name", NAMES, false)
            .field("`--search` (`-s`)", SEARCHING, false)
            .field("`form` (`f`)", "After an ID or a name, picks a form: 1-4, n e t u, f c s u, or Normal, Evolved, True, Ultra", false),
        ("cat", "stats") => Voiced::new(prefix, "hg!cat (c) • Stats", "`hg!cat <unit> [s | stat] ...`")
            .field("Level", "`lv50+70`, `lvl50`, `l50`, `level50` or just `50`. Without one, a typical level for its rarity, raised to the level its form or its ultra talents unlock at if that's higher.", false)
            .field("`--talents` (`-t`)", "`all` (`a`), `standard` (`s`), `ultra` (`u`), `none` (`n`), `show` (`sh`) to list them with their max levels, levels in order like `10 10 5`, or names like `Attack Buff 5`. Talents lift a Normal or Evolved form to True.", false),
        ("cat", "animation") => Voiced::new(prefix, "hg!cat (c) • Animation", "`hg!cat <unit> [a | anim] [type] ...`").field("Types", CLIPS, false).field("Flags", MOTION_FLAGS, false),
        ("cat", "buttons") => Voiced::new(prefix, "hg!cat (c) • Buttons", "Statblocks get a button per form, Level, and Talents on True and Ultra forms. Level and Talents turn green when you've set them. Only the person who asked can press them."),
        ("cat", _) => Voiced::new(prefix, "hg!cat (c)", "`hg!cat <unit> [stats | animation] ...`")
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a level or `--talents` means stats. `s` or `a` still decides outright, and stats is the default.", false)
            .field("Examples", "`hg!c 137_u lv50+70 -t all`\n`hg!c bahamut cat form normal`\n`hg!c jamiera cat a showcase -f webp`\n`hg!c 008 a walk -l -r 10~120`", false),
        ("enemy", "unit") => Voiced::new(prefix, "hg!enemy (e) • Unit", "What goes where `<unit>` is in `hg!enemy <unit> ...`")
            .field("ID", "`763` or `0763`", false)
            .field("Name", NAMES, false)
            .field("`--search` (`-s`)", SEARCHING, false),
        ("enemy", "stats") => Voiced::new(prefix, "hg!enemy (e) • Stats", "`hg!enemy <unit> [s | stat] ...`").field("Magnification", "`400%`, or `400%/200%` for hitpoints and attack. 100% without one.", false),
        ("enemy", "animation") => Voiced::new(prefix, "hg!enemy (e) • Animation", "`hg!enemy <unit> [a | anim] [type] ...`").field("Types", CLIPS, false).field("Flags", MOTION_FLAGS, false),
        ("enemy", _) => Voiced::new(prefix, "hg!enemy (e)", "`hg!enemy <unit> [stats | animation] ...`")
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a percentage means stats. `s` or `a` still decides outright, and stats is the default.", false)
            .field("Examples", "`hg!e akaza 400%`, `hg!e akaza s 400`\n`hg!e 763 a attack -c -w`", false),
        ("stage", _) => Voiced::new(prefix, "hg!stage (s)", "`hg!stage <name> ...`")
            .field("What you get", prose::PAGE_STAGE.said(prose), false)
            .field("Name", "Spaces are fine, and close spellings still match. A name several stages share gets a list to pick from.\n`--search` (`-s`) after it lists the closest matches, 24 to a page, and a number after that, 1 to 100, is how close a name has to be", false)
            .field("ID", "A category's prefix, the map and the stage: `R 001 02`, `R-1-2`, `r001_02` and `R1 2` are all the same stage. A file's prefix works too, so `RN 0 7` is `N 0 7` and `EX 70 9` is `RE 70 9`.", false)
            .field("`--crowns 2` (`-c`)", "1 to 4, and the lowest one the stage is open at without it. A stage that doesn't have the one you ask for says so. A stage with more than one also gets a button each under the picture; only the person who asked can press them.", false)
            .field("Examples", "`hg!stage earthshaker`\n`hg!s sleeping lion -c 4`\n`hg!s moon -s`", false),
        ("map", _) => Voiced::new(prefix, "hg!map (m)", "`hg!map <name | ID> ...`")
            .field("What you get", prose::PAGE_MAP.said(prose), false)
            .field("Name", "Spaces are fine, and close spellings still match. A name several maps share gets a list to pick from, each with its category and top crown.\n`--search` (`-s`) after it lists the closest matches, 24 to a page", false)
            .field("ID", "A category's prefix and the map: `R 100`, `R-100`, `r100`. A file's prefix works too, so `RN 0` is `N 0`.", false)
            .field("`--crowns 2` (`-c`)", "1 to 4. The stage you pick comes at that crown, and says so if it doesn't have it.", false)
            .field("Stages", "The list holds 24 stages at a time. ◀ and ▶ under it turn its pages, and the page number between them opens a box to jump to one.", false)
            .field("Invasions", "An invasion is a map of its own, named after the map it invades and marked Invasion. So `hg!map laboratory of relics` offers the map and its invasion, and the Filibuster's is Cats of the Cosmos Ch. 3 with every stage invaded.", false)
            .field("Examples", "`hg!map the legend begins`\n`hg!m N 0 -c 4`\n`hg!m empire of cats ch. 1`", false),
        ("background", _) => Voiced::new(prefix, "hg!background (bg, b)", "`hg!background <ID | stage | map> ...`")
            .field("What you get", prose::PAGE_BACKGROUND.said(prose), false)
            .field("Flags", "`--seed 100` (`-e`): which random run a moving background plays, 100 to start\n`--range 0~299` or `--range 0 299` (`-r`): frames of a moving background, 0~299 to start\n`--offset 100 50` (`-o`): moves the view right and up from the middle of the screen; negative numbers go left and down\n`--dimensions 800x600` (`-d`): the picture's size, growing outward from that middle; `800 600` and `800×600` work too. 960x640 to start, up to 4096 on a side and 4 megapixels in all\n`--format webp` (`-f`): `avif` (`a`), `webp` (`w`) or `gif` (`g`), this time only, over your default\n`--quality 90` (`-q`): 0 to 100, 80 to start\n`--compress 30` (`-p`): 0 to 100, 0 to start; higher takes longer", false)
            .field("Stage or map", "A stage's name, or its ID like `N 0 7`, finds it the way `hg!stage` does and sends the background that stage uses.\nA map's name, or its ID like `N 0`, lists its stages the way `hg!map` does, and the one you pick sends its background. Your flags carry over.\nStages and maps are looked through together, and `--search` (`-s`) works here too.", false)
            .field("Examples", "`hg!bg 0`\n`hg!bg 12 -f gif -d 1920x640`\n`hg!bg 197 -e 7 -r 0~149`\n`hg!bg 2 -o 400 100 -d 800x600`\n`hg!bg earthshaker -f webp`\n`hg!bg the legend begins`", false),
        ("audio", _) => Voiced::new(prefix, "hg!audio (a)", "`hg!audio <ID | stage | map> ...`")
            .field("What you get", prose::PAGE_AUDIO.said(prose), false)
            .field("Stage or map", "A stage's name, or its ID like `N 0 7`, finds it the way `hg!stage` does and sends the music it starts with.\nA map's name, or its ID like `N 0`, lists its stages the way `hg!map` does, and the one you pick sends its music.\nStages and maps are looked through together, `--search` (`-s`) included.", false)
            .field("`--boss` (`-b`)", "Sends the stage's boss theme instead. A stage that has both gets Standard and Boss buttons under the track; only the person who asked can press them.", false)
            .field("Examples", "`hg!audio 5`\n`hg!a 7`\n`hg!a earthshaker`\n`hg!a earthshaker -b`\n`hg!a the legend begins`", false),
        ("settings", _) => Voiced::new(prefix, "hg!settings (set)", prose::PAGE_SETTINGS.said(prose))
            .field("Language", "Every language the game data has. It also changes which names searches match. English to start.", false)
            .field("Format", "AVIF, WebP or GIF, for animations. AVIF to start, and the smallest by far. `--format` overrides it for one request.", false)
            .field("Default form", "The form a bare ID gets, or a name that matches several forms of one cat. True to start; a cat without it gets the next form down.", false)
            .field("Search", "Exact (to start): a form's exact name gets that form, anything else is a close search. Preferred: a found cat always gets your default form.", false)
            .field("Prefix", "What you type in front of a command, which is `hg!` for you right now. Up to 16 characters, no spaces; leave it empty to go back to `{default}`. Mentioning me and `/` always work.", false)
            .field("Prose", "Human (to start) or Tsundere: how I word what I say. What I tell you stays the same.", false),
        ("website", _) => Voiced::new(prefix, "hg!website (web, w)", prose::PAGE_WEBSITE.said(prose)),
        ("invite", _) => Voiced::new(prefix, "hg!invite (inv, i)", prose::PAGE_INVITE.said(prose)),
        ("legal", _) => Voiced::new(prefix, "hg!legal (license, l)", prose::PAGE_LEGAL.said(prose)),
        ("help", _) => Voiced::new(prefix, "hg!help (h)", prose::PAGE_HELP.said(prose)),
        _ => Voiced::new(prefix, "Hagane", prose::HELP_HOME.said(prose))
            .field("`hg!cat` (`c`)", prose::HOME_CAT.said(prose), false)
            .field("`hg!enemy` (`e`)", prose::HOME_ENEMY.said(prose), false)
            .field("`hg!stage` (`s`)", prose::HOME_STAGE.said(prose), false)
            .field("`hg!map` (`m`)", prose::HOME_MAP.said(prose), false)
            .field("`hg!background` (`bg`, `b`)", prose::HOME_BACKGROUND.said(prose), false)
            .field("`hg!audio` (`a`)", prose::HOME_AUDIO.said(prose), false)
            .field("`hg!settings` (`set`)", prose::HOME_SETTINGS.said(prose), false)
            .field("`hg!website` (`web`, `w`)", prose::HOME_WEBSITE.said(prose), false)
            .field("`hg!invite` (`inv`, `i`)", prose::HOME_INVITE.said(prose), false)
            .field("`hg!legal` (`license`, `l`)", prose::HOME_LEGAL.said(prose), false)
            .field("`hg!help` (`h`)", prose::HOME_HELP.said(prose), false),
    };

    voiced.embed
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("web", "w"))]
async fn website(ctx: Context<'_>) -> Result<(), Error> {
    say(ctx, prose::WEBSITE.said(prose_of(ctx))).await?;
    Ok(())
}

#[poise::command(prefix_command)]
async fn donothing(ctx: Context<'_>) -> Result<(), Error> {
    say(ctx, prose::DO_NOTHING.said(prose_of(ctx))).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("inv", "i"))]
async fn invite(ctx: Context<'_>) -> Result<(), Error> {
    say(ctx, prose::INVITE.said(prose_of(ctx))).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("license", "l"))]
async fn legal(ctx: Context<'_>) -> Result<(), Error> {
    send(ctx, file(LICENSES.as_bytes().to_vec(), "licenses.txt".to_owned()).content(LEGAL)).await?;
    Ok(())
}

const FLAGS: [(&str, &str); 9] = [("en", "🇺🇸"), ("ja", "🇯🇵"), ("tw", "🇹🇼"), ("ko", "🇰🇷"), ("es", "🇪🇸"), ("de", "🇩🇪"), ("fr", "🇫🇷"), ("it", "🇮🇹"), ("th", "🇹🇭")];

fn flag(code: &str) -> &'static str {
    FLAGS.iter().find(|(held, _)| *held == code).map_or("🌐", |(_, flag)| flag)
}

#[derive(Clone, Copy)]
enum Setting {
    Language,
    Format,
    Form,
    Search,
    Prose,
    Prefix,
}

impl Setting {
    fn key(self) -> &'static str {
        match self {
            Setting::Language => "lang",
            Setting::Format => "fmt",
            Setting::Form => "form",
            Setting::Search => "search",
            Setting::Prose => "prose",
            Setting::Prefix => "prefix",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Setting::Language => "Language",
            Setting::Format => "Format",
            Setting::Form => "Form",
            Setting::Search => "Search",
            Setting::Prose => "Prose",
            Setting::Prefix => "Prefix",
        }
    }

    fn row(self, owner: u64, prefs: &Prefs) -> serenity::CreateActionRow {
        let key = self.key();
        let choices: Vec<serenity::CreateButton> = match self {
            Setting::Language => {
                let options = lang::choices()
                    .map(|(code, name)| serenity::CreateSelectMenuOption::new(name, code).emoji(serenity::ReactionType::Unicode(flag(code).to_owned())).default_selection(code == prefs.language))
                    .collect();

                return serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(format!("set|{owner}|{key}"), serenity::CreateSelectMenuKind::String { options }).placeholder(self.label()));
            }
            Setting::Format => [Format::Avif, Format::WebP, Format::Gif].into_iter().map(|format| pick(format == prefs.format, format!("set|{owner}|{key}|{}", format.extension()), format.label())).collect(),
            Setting::Form => FORMS.iter().enumerate().map(|(form, label)| pick(form == prefs.form, format!("set|{owner}|{key}|{form}"), label)).collect(),
            Setting::Search => [Search::Exact, Search::Preferred].into_iter().map(|search| pick(search == prefs.search, format!("set|{owner}|{key}|{}", search.slug()), search.label())).collect(),
            Setting::Prose => Prose::ALL.into_iter().map(|prose| pick(prose == prefs.prose, format!("set|{owner}|{key}|{}", prose.slug()), prose.label())).collect(),
            Setting::Prefix => vec![pick(prefs.prefix.is_some(), format!("set|{owner}|{key}"), "Change")],
        };

        serenity::CreateActionRow::Buttons(std::iter::once(caption(format!("set|{owner}|caption|{key}"), self.label())).chain(choices).collect())
    }
}

fn caption(custom: String, label: &str) -> serenity::CreateButton {
    serenity::CreateButton::new(custom).label(label).style(serenity::ButtonStyle::Secondary).disabled(true)
}

fn pick(current: bool, custom: String, label: &str) -> serenity::CreateButton {
    serenity::CreateButton::new(custom).label(label).style(if current { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary })
}

fn settings_view(owner: u64, prefs: &Prefs, shown: usize) -> (serenity::CreateEmbed, Vec<serenity::CreateActionRow>) {
    let form = FORMS.get(prefs.form).copied().unwrap_or("True");
    let embed = page(prose::SETTINGS_TITLE.said(prefs.prose), prose::SETTINGS_NOTE.said(prefs.prose))
        .field("Language", format!("{} {}", flag(&prefs.language), prefs::language_name(&prefs.language)), true)
        .field("Format", prefs.format.label(), true)
        .field("Default form", form, true)
        .field("Search", prefs.search.label(), true)
        .field("Prefix", prefs.prefix(), true)
        .field("Prose", prefs.prose.label(), true);
    let shown = shown.min(SETTINGS_PAGES - 1);
    let mut rows: Vec<serenity::CreateActionRow> = SETTINGS.chunks(SETTINGS_ROWS).nth(shown).unwrap_or_default().iter().map(|setting| setting.row(owner, prefs)).collect();

    for blank in rows.len()..SETTINGS_ROWS {
        rows.push(serenity::CreateActionRow::Buttons(vec![caption(format!("set|{owner}|blank|{blank}"), BLANK)]));
    }

    let turn = |to: Option<usize>, side: &str, label: &str| {
        serenity::CreateButton::new(format!("set|{owner}|page|{}", to.map_or_else(|| side.to_owned(), |to| to.to_string()))).label(label).style(serenity::ButtonStyle::Secondary).disabled(to.is_none())
    };

    rows.push(serenity::CreateActionRow::Buttons(vec![
        turn(shown.checked_sub(1), "first", "◀"),
        serenity::CreateButton::new(format!("set|{owner}|jump|{shown}")).label(format!("{} / {SETTINGS_PAGES}", shown + 1)).style(serenity::ButtonStyle::Secondary),
        turn(Some(shown + 1).filter(|next| *next < SETTINGS_PAGES), "last", "▶"),
    ]));

    (embed, rows)
}

fn settings_page(what: &str) -> usize {
    SETTINGS.iter().position(|setting| setting.key() == what).map_or(0, |slot| slot / SETTINGS_ROWS)
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("set"))]
async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    if let poise::Context::Prefix(_) = ctx {
        let open = serenity::CreateButton::new(OPEN_SETTINGS).label("Open Settings").style(serenity::ButtonStyle::Primary);

        send(ctx, CreateReply::default().content(prose::SETTINGS_PROMPT.said(prose_of(ctx))).components(vec![serenity::CreateActionRow::Buttons(vec![open])])).await?;
        return Ok(());
    }

    let owner = ctx.author().id.get();
    let (embed, rows) = settings_view(owner, &ctx.data().book.get(owner), 0);

    send(ctx, CreateReply::default().embed(embed).components(rows).ephemeral(true)).await?;
    Ok(())
}

async fn on_event(ctx: &serenity::Context, event: &serenity::FullEvent, data: &Data) -> Result<(), Error> {
    match event {
        serenity::FullEvent::GuildCreate { guild, .. } if !whitelist::admits(guild.id.get()) => {
            warn!(guild = guild.id.get(), name = guild.name, "Leaving a server that is not on the whitelist");
            tell_unlisted(ctx, guild).await;

            if let Err(err) = guild.id.leave(ctx).await {
                error!(guild = guild.id.get(), "Could not leave a server that is not on the whitelist: {}", err);
            }
        }
        serenity::FullEvent::InteractionCreate { interaction: serenity::Interaction::Component(press) } => on_press(ctx, press, data).await?,
        serenity::FullEvent::InteractionCreate { interaction: serenity::Interaction::Modal(form) } => on_modal(ctx, form, data).await?,
        _ => {}
    }

    Ok(())
}

async fn tell_unlisted(ctx: &serenity::Context, guild: &serenity::Guild) {
    let mut text: Vec<&serenity::GuildChannel> = guild.channels.values().filter(|channel| channel.kind == serenity::ChannelType::Text).collect();

    text.sort_by_key(|channel| (channel.position, channel.id));

    let mut tried: Vec<serenity::ChannelId> = Vec::new();

    for channel in guild.system_channel_id.into_iter().chain(text.iter().map(|channel| channel.id)) {
        if tried.contains(&channel) {
            continue;
        }

        if tried.len() >= NOTICE_TRIES {
            break;
        }

        tried.push(channel);

        if channel.say(ctx, prose::UNLISTED.said(Prose::Human)).await.is_ok() {
            return;
        }
    }

    warn!(guild = guild.id.get(), "No channel would take the whitelist notice");
}

async fn refuse(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data) -> Result<(), Error> {
    let notice = serenity::CreateInteractionResponseMessage::new().content(prose::NOT_YOURS.said(data.book.prose(press.user.id.get()))).ephemeral(true);

    press.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
    Ok(())
}

async fn on_press(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data) -> Result<(), Error> {
    let custom = press.data.custom_id.as_str();

    if custom == OPEN_SETTINGS {
        let owner = press.user.id.get();
        let (embed, rows) = settings_view(owner, &data.book.get(owner), 0);
        let private = serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows).ephemeral(true);

        press.create_response(ctx, serenity::CreateInteractionResponse::Message(private)).await?;
        return Ok(());
    }

    if let Some(rest) = custom.strip_prefix("set|") {
        return on_setting(ctx, press, data, rest).await;
    }

    if let Some(rest) = custom.strip_prefix(FIND) {
        return on_found(ctx, press, data, rest).await;
    }

    if let Some(rest) = custom.strip_prefix(CROWN) {
        return on_crown(ctx, press, data, rest).await;
    }

    if let Some(rest) = custom.strip_prefix(THEME_PRESS) {
        return on_theme(ctx, press, data, rest).await;
    }

    if let Some(rest) = custom.strip_prefix(CHART) {
        return on_chart(ctx, press, data, rest).await;
    }

    if let Some(rest) = custom.strip_prefix(SEEK) {
        return on_seek(ctx, press, data, rest).await;
    }

    if let Some((owner, page)) = custom.strip_prefix("help|").and_then(|rest| rest.split_once('|')) {
        if owner.parse::<u64>().ok() != Some(press.user.id.get()) {
            return refuse(ctx, press, data).await;
        }

        let (page, section) = page.split_once('|').unwrap_or((page, ""));
        let page = canonical(page).unwrap_or(HOME);
        let section = section_of(page, section);
        let turned = serenity::CreateInteractionResponseMessage::new().embed(help_page(page, section, &prefix_of(data, press.user.id.get()), data.book.prose(press.user.id.get()))).components(help_rows(press.user.id.get(), page, section));

        press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    let Some((tag, sheet)) = Sheet::read(custom) else { return Ok(()) };

    if press.user.id.get() != sheet.owner {
        return refuse(ctx, press, data).await;
    }

    match tag {
        "sb" => {
            press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

            if let Some(edit) = redrawn(data, &sheet).await? {
                press.edit_response(ctx, edit).await?;
            }

            Ok(())
        }
        "st" => {
            let current = sheet.talents.as_deref().map(|levels| levels.replace('.', " ")).unwrap_or_default();
            let input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, "Talent levels (none or empty turns them off)", "levels").placeholder(TALENT_HINT).value(current).required(false);
            let modal = serenity::CreateModal::new(sheet.custom("sm", sheet.form), "Talents").components(vec![serenity::CreateActionRow::InputText(input)]);

            press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
            Ok(())
        }
        "sl" => {
            let prefs = data.book.get(sheet.owner);
            let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };
            let Some(cat) = library.cat(sheet.id) else { return Ok(()) };
            let mut input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, "Level (empty goes back to automatic)", "level").placeholder(library.level_for(cat, sheet.form, sheet.talents.as_deref().and_then(talents::decoded).as_ref())).max_length(LEVEL_LIMIT).required(false);

            if let Some(level) = sheet.level.as_deref() {
                input = input.value(level);
            }

            let modal = serenity::CreateModal::new(sheet.custom("sv", sheet.form), "Level").components(vec![serenity::CreateActionRow::InputText(input)]);

            press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn typed(form: &serenity::ModalInteraction) -> String {
    form.data
        .components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            serenity::ActionRowComponent::InputText(input) => input.value.clone(),
            _ => None,
        })
        .unwrap_or_default()
}

async fn on_prefix(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data) -> Result<(), Error> {
    let owner = form.user.id.get();
    let response = match prefs::prefix_from(&typed(form)) {
        Ok(prefix) => {
            let prefs = data.book.update(owner, |prefs| prefs.prefix = prefix);
            let (embed, rows) = settings_view(owner, &prefs, settings_page("prefix"));

            serenity::CreateInteractionResponse::UpdateMessage(serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows))
        }
        Err(line) => {
            let reason = line.filled(data.book.prose(owner), &[("limit", &prefs::PREFIX_LIMIT.to_string())]);

            serenity::CreateInteractionResponse::Message(serenity::CreateInteractionResponseMessage::new().content(reason).ephemeral(true))
        }
    };

    form.create_response(ctx, response).await?;
    Ok(())
}

async fn on_level(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data, mut sheet: Sheet, typed: &str) -> Result<(), Error> {
    let typed: String = typed.split_whitespace().collect();
    let level = grammar::level_of(&typed);

    if !typed.is_empty() && level.is_none() {
        let notice = serenity::CreateInteractionResponseMessage::new().content(prose::NOT_A_LEVEL.said(data.book.prose(sheet.owner))).ephemeral(true);

        form.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
        return Ok(());
    }

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;
    sheet.level = level;

    if let Some(edit) = redrawn(data, &sheet).await? {
        form.edit_response(ctx, edit).await?;
    }

    Ok(())
}

async fn on_modal(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data) -> Result<(), Error> {
    if form.data.custom_id == SET_PREFIX {
        return on_prefix(ctx, form, data).await;
    }

    if let Some(rest) = form.data.custom_id.strip_prefix(SEEK_JUMP) {
        let owner = form.user.id.get();
        let (token, shown) = rest.split_once('|').map_or((None, None), |(token, shown)| (u64::from_str_radix(token, 16).ok(), shown.parse::<usize>().ok()));
        let held = token.and_then(|token| lists(data).find(token, owner));
        let (Some(token), Some(shown), Some(choices)) = (token, shown, held) else {
            let gone = serenity::CreateInteractionResponseMessage::new().content(prose::LIST_GONE.said(data.book.prose(owner))).ephemeral(true);

            form.create_response(ctx, serenity::CreateInteractionResponse::Message(gone)).await?;
            return Ok(());
        };
        let asked = typed(form).trim().parse::<usize>().ok().map_or(shown, |page| page.clamp(1, list_pages(&choices)) - 1);
        let turned = serenity::CreateInteractionResponseMessage::new().components(list_rows(owner, Some(token), &choices, asked));

        form.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    if let Some(rest) = form.data.custom_id.strip_prefix(CHART_JUMP) {
        let mut parts = rest.splitn(4, '|');
        let (Some(route), Some(code), Some(shown), Some(tail)) = (parts.next(), parts.next(), parts.next().and_then(|shown| shown.parse::<usize>().ok()), parts.next()) else { return Ok(()) };
        let owner = form.user.id.get();
        let prefs = data.book.get(owner);
        let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };
        let pages = chart_pages(&library, code).unwrap_or(1);
        let asked = typed(form).trim().parse::<usize>().ok().map_or(shown, |page| page.clamp(1, pages) - 1);
        let Ok(post) = chart(owner, &library, code, asked, route, tail, prefs.prose) else { return Ok(()) };
        let turned = serenity::CreateInteractionResponseMessage::new().content(post.text).components(post.rows);

        form.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    if let Some(shown) = form.data.custom_id.strip_prefix(SET_JUMP) {
        let owner = form.user.id.get();
        let asked = typed(form).trim().parse::<usize>().ok().map(|page| page.clamp(1, SETTINGS_PAGES) - 1);
        let (embed, rows) = settings_view(owner, &data.book.get(owner), asked.or_else(|| shown.parse().ok()).unwrap_or(0));
        let turned = serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows);

        form.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    let Some((tag, mut sheet)) = Sheet::read(&form.data.custom_id) else { return Ok(()) };

    if form.user.id.get() != sheet.owner {
        return Ok(());
    }

    let typed = typed(form);

    match tag {
        "sv" => return on_level(ctx, form, data, sheet, &typed).await,
        "sm" => {}
        _ => return Ok(()),
    }

    let prefs = data.book.get(sheet.owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };
    let Some(cat) = library.cat(sheet.id) else { return Ok(()) };
    let words: Vec<&str> = typed.split_whitespace().collect();

    if talents::shows(&grammar::talents_of(&words)) {
        let private = serenity::CreateInteractionResponseMessage::new().content(talents::overview(&library, cat, sheet.form, prefs.prose)).ephemeral(true);

        form.create_response(ctx, serenity::CreateInteractionResponse::Message(private)).await?;
        return Ok(());
    }

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    sheet.talents = if words.is_empty() {
        None
    } else {
        match talents::levels(&library, cat, &grammar::talents_of(&words)) {
            Ok(levels) => talents::encoded(&levels, cat.talent_data.as_ref().map_or(0, |held| held.groups.len())),
            Err(trouble) => {
                let notice = serenity::CreateInteractionResponseFollowup::new().content(format!("{}\n{}", trouble.told(prefs.prose), talents::listed(&library, cat, None))).ephemeral(true);

                form.create_followup(ctx, notice).await?;
                return Ok(());
            }
        }
    };

    if let Some(edit) = redrawn(data, &sheet).await? {
        form.edit_response(ctx, edit).await?;
    }

    Ok(())
}

async fn redrawn(data: &Data, sheet: &Sheet) -> Result<Option<serenity::EditInteractionResponse>, Error> {
    let prefs = data.book.get(sheet.owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(None) };
    let rendered = {
        let (library, sheet) = (Arc::clone(&library), sheet.clone());

        let cache = Arc::clone(&data.cache);

        blocking(data, move || sheet_png(&library, &cache, &sheet)).await?
    };

    match rendered {
        Ok((bytes, name)) => {
            let attachments = serenity::EditAttachments::new().add(serenity::CreateAttachment::bytes(bytes, name));

            Ok(Some(serenity::EditInteractionResponse::new().attachments(attachments).components(sheet_rows(&library, sheet))))
        }
        Err(reason) => {
            error!(cat = sheet.id, "A statblock could not be redrawn: {}", reason.told(Prose::Human));
            Ok(None)
        }
    }
}

async fn on_setting(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.split('|');
    let Some(owner) = parts.next().and_then(|owner| owner.parse::<u64>().ok()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    let what = parts.next().unwrap_or_default();

    if what == "prefix" {
        let mut input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, "Prefix (empty goes back to hg!)", "prefix").placeholder(prefs::PREFIX).max_length(prefs::PREFIX_LIMIT as u16).required(false);

        if let Some(chosen) = data.book.prefix(owner) {
            input = input.value(chosen);
        }

        let modal = serenity::CreateModal::new(SET_PREFIX, "Prefix").components(vec![serenity::CreateActionRow::InputText(input)]);

        press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
        return Ok(());
    }

    if what == "jump" {
        let shown = parts.next().and_then(|page| page.parse::<usize>().ok()).unwrap_or(0);
        let input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, format!("Page (1 to {SETTINGS_PAGES})"), "page").placeholder((shown + 1).to_string()).max_length(PAGE_DIGITS).required(false);
        let modal = serenity::CreateModal::new(format!("{SET_JUMP}{shown}"), "Go to page").components(vec![serenity::CreateActionRow::InputText(input)]);

        press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
        return Ok(());
    }

    let value = match &press.data.kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => values.first().cloned().unwrap_or_default(),
        _ => parts.next().unwrap_or_default().to_owned(),
    };
    let page = if what == "page" { value.parse().unwrap_or(0) } else { settings_page(what) };
    let prefs = data.book.update(owner, |prefs| match what {
        "lang" if lang::choices().any(|(code, _)| code == value) => prefs.language = value,
        "fmt" => prefs.format = [Format::Avif, Format::WebP, Format::Gif].into_iter().find(|format| format.extension() == value).unwrap_or(prefs.format),
        "form" => prefs.form = value.parse::<usize>().ok().filter(|form| *form < FORMS.len()).unwrap_or(prefs.form),
        "search" => prefs.search = [Search::Exact, Search::Preferred].into_iter().find(|search| search.slug() == value).unwrap_or(prefs.search),
        "prose" => prefs.prose = Prose::ALL.into_iter().find(|prose| prose.slug() == value).unwrap_or(prefs.prose),
        _ => {}
    });
    let (embed, rows) = settings_view(owner, &prefs, page);
    let updated = serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows);

    press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(updated)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: serenity::UserId = serenity::UserId::new(42);

    // DMs and a person's own install are open to everyone, but only listed users get something new encoded there.
    #[test]
    fn personal_use_answers_but_only_encodes_for_listed_users() {
        let owner = serenity::UserId::new(1360862647578394704);
        let listed = Some(serenity::GuildId::new(1456204440951455786));
        let elsewhere = Some(serenity::GuildId::new(7));
        let by_server = serenity::AuthorizingIntegrationOwners(vec![serenity::AuthorizingIntegrationOwner::GuildInstall(elsewhere), serenity::AuthorizingIntegrationOwner::UserInstall(ME)]);
        let by_user = serenity::AuthorizingIntegrationOwners(vec![serenity::AuthorizingIntegrationOwner::UserInstall(ME)]);

        assert!(personal(None, &by_server), "a DM is personal however she was installed");
        assert!(personal(elsewhere, &by_user), "her own install in someone else's server is personal");
        assert!(!personal(elsewhere, &by_server), "a server that installed her is not");
        assert_eq!(refusal(None, true), None);
        assert_eq!(refusal(listed, false), None);
        assert_eq!(refusal(elsewhere, false), Some(&prose::UNLISTED));
        assert_eq!(refusal(elsewhere, true), None, "the whitelist is for servers that install her");
        assert!(encodes_for(false, ME));
        assert!(encodes_for(true, owner));
        assert!(!encodes_for(true, ME));
    }
    const SOMEONE: serenity::UserId = serenity::UserId::new(7);

    // Poise wants the prefix itself back and strips it on its own, so check what it is left with.
    fn rest(content: &str, direct: bool, replied_to: Option<serenity::UserId>) -> Option<&str> {
        prefix_length(content, prefs::PREFIX, ME, direct, replied_to).map(|length| content[length..].trim_start())
    }

    #[test]
    fn every_way_in_reaches_the_command() {
        assert_eq!(rest("hg!cat 25", false, None), Some("cat 25"));
        assert_eq!(rest("Hg! cat 25", false, None), Some("cat 25"));
        assert_eq!(rest("  hg!cat 25", false, None), Some("cat 25"));
        assert_eq!(rest("<@42> cat 25", false, None), Some("cat 25"));
        assert_eq!(rest("<@!42>cat 25", false, None), Some("cat 25"));
        assert_eq!(rest("cat 25", true, None), Some("cat 25"));
        assert_eq!(rest("cat 25", false, Some(ME)), Some("cat 25"));
        assert_eq!(prefix_length("hg!cat 25", prefs::PREFIX, ME, false, None).map(|length| &"hg!cat 25"[..length]), Some("hg!"));
    }

    // Every settings page is the same height, and the arrows stop at the ends.
    #[test]
    fn settings_pages_keep_their_shape() {
        let rows = |page: usize| serde_json::to_value(settings_view(1, &Prefs::default(), page).1).unwrap();
        let ids = |rows: &serde_json::Value| -> Vec<String> { rows.as_array().unwrap().iter().flat_map(|row| row["components"].as_array().unwrap().iter().map(|part| part["custom_id"].as_str().unwrap().to_owned())).collect() };

        for page in 0..SETTINGS_PAGES {
            let rows = rows(page);
            let held = ids(&rows);
            let turns = rows[SETTINGS_ROWS]["components"].as_array().unwrap();

            assert_eq!(rows.as_array().unwrap().len(), SETTINGS_ROWS + 1, "page {page}");
            assert!(rows.as_array().unwrap().iter().all(|row| (1..=5).contains(&row["components"].as_array().unwrap().len())));
            assert_eq!(held.iter().collect::<std::collections::HashSet<_>>().len(), held.len(), "page {page} repeats an id");
            assert_eq!(turns[0]["disabled"].as_bool().unwrap_or(false), page == 0);
            assert_eq!(turns[1]["label"], format!("{} / {SETTINGS_PAGES}", page + 1));
            assert_eq!(turns[2]["disabled"].as_bool().unwrap_or(false), page == SETTINGS_PAGES - 1);
        }

        assert_eq!(rows(9), rows(SETTINGS_PAGES - 1), "a page past the end is the last one");
    }

    // The list of settings alone decides the pages: four to a page, the gaps padded, and each one found where it was put.
    #[test]
    fn settings_fill_their_pages_in_order() {
        let rows = |page: usize| serde_json::to_value(settings_view(1, &Prefs::default(), page).1).unwrap();

        assert_eq!(SETTINGS_PAGES, SETTINGS.len().div_ceil(SETTINGS_ROWS));

        for (slot, setting) in SETTINGS.iter().enumerate() {
            let row = &rows(slot / SETTINGS_ROWS)[slot % SETTINGS_ROWS]["components"];
            let lead = row[0]["custom_id"].as_str().unwrap();

            assert!(lead.ends_with(setting.key()), "{} is not in slot {slot}", setting.label());
            assert_eq!(settings_page(setting.key()), slot / SETTINGS_ROWS);
        }

        for page in 0..SETTINGS_PAGES {
            let filled = SETTINGS.chunks(SETTINGS_ROWS).nth(page).unwrap().len();
            let rows = rows(page);

            for row in filled..SETTINGS_ROWS {
                assert_eq!(rows[row]["components"][0]["label"], BLANK);
                assert_eq!(rows[row]["components"][0]["disabled"], true);
            }

            for row in 0..filled {
                assert_ne!(rows[row]["components"][0]["label"], BLANK);
            }
        }

        assert_eq!(settings_page("page"), 0, "anything that is not a setting opens the first page");
    }

    // Help speaks in the reader's own prefix, and only names hg! where it means the default.
    #[test]
    fn help_uses_the_readers_prefix() {
        let voiced = |page: &str, section: &str, prefix: &str, prose: Prose| serde_json::to_string(&help_page(page, section, prefix, prose)).unwrap();
        let shown = |page: &str, prefix: &str| voiced(page, section_of(page, ""), prefix, Prose::Human);

        for (page, _) in PAGES {
            for section in sections(page).iter().map(|(key, _)| *key).chain([""]) {
                for prose in Prose::ALL {
                    assert!(!voiced(page, section, "hg!", prose).contains(DEFAULT_PREFIX), "{page} {section}");
                    assert_eq!(voiced(page, section, "hagane", prose).matches("hg!").count(), usize::from(page == "settings"), "{page} {section}");
                }
            }
        }

        assert!(voiced(HOME, "", "hagane", Prose::Tsundere).contains("Use `hagane`, mention me, reply to me, DM me, or use `/`. I-It's not like I'm happy when you do!"));

        assert!(shown("cat", "hagane").contains("`haganecat <unit>"));
        assert!(shown("cat", "hg!").contains("`hg!cat <unit>"));
        assert!(shown("settings", "hagane").contains("go back to `hg!`"));
    }

    // Cat and Enemy open onto their own section buttons with Home beside them, every other page keeps the page buttons, and nothing overflows Discord's limits.
    #[test]
    fn help_pages_with_sections_swap_their_buttons() {
        let labels = |page: &str, section: &str| -> Vec<Vec<String>> {
            serde_json::to_value(help_rows(1, page, section)).unwrap().as_array().unwrap().iter().map(|row| row["components"].as_array().unwrap().iter().map(|button| button["label"].as_str().unwrap().to_owned()).collect()).collect()
        };

        assert_eq!(labels("cat", GENERAL), [["Home", "General", "Unit"], ["Stats", "Animation", "Buttons"]]);
        assert_eq!(labels("enemy", "stats"), [["Home", "General", "Unit", "Stats", "Animation"]]);
        assert_eq!(labels(HOME, ""), [["Home", "Cat", "Enemy", "Stage"], ["Map", "Background", "Audio", "Settings"], ["Website", "Invite", "Legal", "Help"]]);
        assert_eq!(labels("audio", ""), labels(HOME, ""));
        assert_eq!((section_of("cat", ""), section_of("cat", "stats"), section_of("cat", "anim"), section_of("cat", "nonsense"), section_of("enemy", "buttons"), section_of("audio", "unit")), (GENERAL, "stats", "animation", GENERAL, GENERAL, ""));

        for (page, _) in PAGES {
            for section in sections(page).iter().map(|(key, _)| *key).chain([""]) {
                let embed = serde_json::to_value(help_page(page, section_of(page, section), "hg!", Prose::Tsundere)).unwrap();
                let fields = embed["fields"].as_array().cloned().unwrap_or_default();

                assert!(embed["description"].as_str().is_some_and(|text| !text.is_empty() && text.len() <= 4096), "{page} {section}");
                assert!(fields.iter().all(|field| field["value"].as_str().is_some_and(|text| !text.is_empty() && text.len() <= 1024)), "{page} {section}");
                assert!(help_rows(1, page, section).len() <= 5);
            }
        }
    }

    // Someone's own prefix takes the place of hg!, which then reads as plain chat for them.
    #[test]
    fn a_chosen_prefix_replaces_the_default() {
        let chosen = |content: &'static str| prefix_length(content, "hagane", ME, false, None).map(|length| content[length..].trim_start());

        assert_eq!(chosen("haganecat 25"), Some("cat 25"));
        assert_eq!(chosen("Hagane cat 25"), Some("cat 25"));
        assert_eq!(chosen("<@42> cat 25"), Some("cat 25"));
        assert_eq!(chosen("hg!cat 25"), None);
        assert_eq!(prefix_length("ねこcat", "ねこ", ME, false, None), Some("ねこ".len()));
        assert_eq!(prefix_length("猫", "ねこ", ME, false, None), None);
    }

    // Ordinary chat must never be read as a command.
    #[test]
    fn plain_chat_is_left_alone() {
        assert_eq!(rest("cat 25", false, None), None);
        assert_eq!(rest("cat 25", false, Some(SOMEONE)), None);
        assert_eq!(rest("!hg cat 25", false, None), None);
        assert_eq!(rest("<@7> cat 25", false, None), None);
    }

    // A stage with several crowns gets one button each, numbered with the crown symbol, and the one shown is marked.
    #[test]
    fn crown_buttons_follow_the_stage() {
        let rows = |stage: &Stage, shown: u8| serde_json::to_value(crown_rows(7, "N000-00", stage, shown)).unwrap();
        let open = Stage { max_crowns: 4, target_crowns: -1, ..Stage::default() };
        let fixed = Stage { max_crowns: 3, target_crowns: 2, ..Stage::default() };
        let buttons = rows(&open, 1);
        let held = buttons[0]["components"].as_array().unwrap();

        assert_eq!(held.iter().map(|button| button["label"].as_str().unwrap()).collect::<Vec<_>>(), ["1\u{1F732}", "2\u{1F732}", "3\u{1F732}", "4\u{1F732}"]);
        assert_eq!(held[1]["custom_id"], "crown|7|N000-00|1");
        assert_eq!(held.iter().map(|button| button["style"].as_u64().unwrap()).collect::<Vec<_>>(), [2, 3, 2, 2], "the crown shown is the green one");
        assert_eq!(rows(&fixed, 2)[0]["components"].as_array().unwrap().iter().map(|button| button["disabled"].as_bool().unwrap_or(false)).collect::<Vec<_>>(), [true, true, false]);
        assert!(rows(&Stage { max_crowns: 1, ..Stage::default() }, 0).as_array().unwrap().is_empty());
    }

    // A list that was kept can be turned by the person who asked and nobody else, and the oldest go first once too much is held.
    #[test]
    fn kept_lists_are_found_by_their_owner_until_they_age_out() {
        let choices = |count: usize| -> Arc<[Choice]> { (0..count).map(|at| (at.to_string(), at.to_string(), String::new())).collect() };
        let mut lists = Lists::new(1_795_000_000);
        let first = lists.keep(7, choices(30));
        let second = lists.keep(8, choices(60));

        assert_ne!(first, second);
        assert_eq!((lists.find(first, 7).map(|held| held.len()), lists.find(second, 8).map(|held| held.len())), (Some(30), Some(60)));
        assert!(lists.find(first, 8).is_none() && lists.find(second + 1, 8).is_none());
        assert_eq!((list_pages(&choices(24)), list_pages(&choices(25)), list_pages(&choices(0))), (1, 2, 1));

        let third = lists.keep(9, choices(LISTS_HELD));

        assert!(lists.find(first, 7).is_none() && lists.find(second, 8).is_none() && lists.find(third, 9).is_some());
        assert!(Lists::new(1_795_000_001).keep(7, choices(1)) > third, "a restart never hands out a token an old list still carries");
        assert!(format!("{SEEK}{}|{:x}|{JUMP}{LIST_PAGES}", u64::MAX, u64::MAX).len() <= VALUE_LIMIT);
    }

    // The pick list's own limits: twenty-four stages and Abort make Discord's twenty-five options.
    #[test]
    fn a_stage_list_page_fits_one_menu() {
        assert_eq!(SEARCH_SHOWN + 1, 25);
        assert!(format!("{}|Space009-47i|-c 4", Kind::Stage.tag()).len() <= VALUE_LIMIT);
        assert!(format!("{CHART}{}|{BACKDROP}|Space009i|{JUMP}99|-f webp -d 1920x1080 -o -200 100 -e 12345", u64::MAX).len() <= VALUE_LIMIT, "a page button carries a full set of background flags");
    }

    #[test]
    fn a_sheet_survives_its_button_id() {
        let sheet = Sheet { owner: 1, id: 25, form: 2, level: Some("50+70".to_owned()), talents: Some("10.0.5".to_owned()) };
        let custom = sheet.custom("sb", 3);
        let (tag, read) = Sheet::read(&custom).unwrap();

        assert_eq!(tag, "sb");
        assert_eq!((read.owner, read.id, read.form, read.level.as_deref(), read.talents.as_deref()), (1, 25, 3, Some("50+70"), Some("10.0.5")));
        assert!(custom.len() <= 100);

        let automatic = Sheet { level: None, talents: None, ..sheet };
        let (_, read) = Sheet::read(&automatic.custom("sl", 3)).unwrap();

        assert_eq!((read.level, read.talents), (None, None), "no level means the automatic one, and it stays that way through a button");
    }
}
