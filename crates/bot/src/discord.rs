use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use poise::serenity_prelude::{self as serenity, Builder};
use poise::{BoxFuture, CreateReply, FrameworkError};
use tokio::sync::{OwnedRwLockReadGuard, RwLock, Semaphore};
use tokio::task;
use tracing::{debug, error, info, warn};

use codex::combat::Foe;
use foundry::common::architecture;
use foundry::domains::cat::animation as cat_animation;
use foundry::domains::cat::level;
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::enemy::animation as enemy_animation;
use foundry::domains::settings::lang;
use foundry::domains::stage::{Stage, sheet};
use foundry::systems::animation::MotionSet;
use foundry::systems::combat::lens::Lens;
use foundry::systems::combat::registry::Magnification;

use super::alias::{self, Aliases, Target};
use super::whitelist;
use super::wiki::Wiki;
use super::audio;
use super::backdrop;
use super::cache::Cache;
use super::config::Config;
use super::fuzzy;
use super::grammar::{self, Backing, Kind, Lookup, Motion, Page, Request, Scenery, Tune, Unit, Versus, Want};
use super::library::{self, Catalog, Library, Remote};
use super::lookup::{self, CROWN_GLYPH, Found, Hit, Landing};
use super::prefs::{self, Book, FORMS, Mode, Prefs, Search};
use super::prose::{self, Line, Prose, Trouble};
use super::render::{self, Format, Options, Planned, Progress};
use super::stage;
use super::statblock;
use super::talents;
use super::usage::{self, Asker, Ledger, Tally};

const REFRESH: Duration = Duration::from_secs(60 * 60);
const WIKI_RETRY: Duration = Duration::from_secs(60 * 60);
const PICK_WAIT: Duration = Duration::from_secs(5 * 60);
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
const MAGNIFICATION_HINT: &str = "400% or 400%/200%";
const LABEL: &str = "label";
const CARD_PAGE: &str = "ep";
const CARD_ASK: &str = "em";
const CARD_SET: &str = "ev";
const TALENT_HINT: &str = "all, standard, ultra, none, 10 10 5 or Attack Buff 5";
const MODE_HELP: &str = "`--mode` is `min` for the plain numbers, `avg` for what the procs average out to (the default) and `max` for every proc landing: crits, savage blows, waves, surges and explosions count toward Attack and DPS outside `min`, and `max` also counts Strengthen and the attack boost from kills.\n`--hits` is `1`, `2` or `3` for one hit alone, `avg` for the hits that reach a foe at standing range (the default) and `all` for every hit.";
const VERSUS_HINT: &str = "red metal, an enemy, targets, traits, subtraits or all";
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
const SET_ALIAS: &str = "set|alias";
const ALIAS_JUMP: &str = "set|aliasjump|";
const ALIAS: &str = "a";
const ALIAS_LINES: usize = 24;
const ALIAS_HINT: &str = "Dasli";
const TARGET_HINT: &str = "Kasli the Bane";
const SETTINGS: [Setting; 8] = [Setting::Language, Setting::Format, Setting::Form, Setting::Search, Setting::Mode, Setting::Prose, Setting::Prefix, Setting::Alias];
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
type Waits = Arc<Mutex<HashMap<u64, Wait>>>;

enum Slip {
    Message(serenity::ChannelId, serenity::MessageId),
    Reply(String),
}

struct Wait {
    due: Instant,
    slip: Slip,
    prose: Prose,
}

pub(crate) struct Data {
    shelf: Shelf,
    cache: Arc<Cache>,
    sheets: Arc<Semaphore>,
    encoder: Arc<Semaphore>,
    book: Arc<Book>,
    lists: Mutex<Lists>,
    usage: Mutex<Ledger>,
    waits: Waits,
    commands: Vec<String>,
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
    let born = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let commands = vec![cat(), enemy(), stage(), map(), background(), audio(), settings(), website(), help(), invite(), legal(), donothing()];
    let data = Data {
        shelf: Arc::clone(&shelf),
        cache: Arc::new(cache),
        sheets: Arc::new(Semaphore::new(1)),
        encoder: Arc::new(Semaphore::new(1)),
        book: Arc::new(Book::open()),
        lists: Mutex::new(Lists::new(born)),
        usage: Mutex::new(Ledger::default()),
        waits: Arc::new(Mutex::new(HashMap::new())),
        commands: commands.iter().flat_map(|command| std::iter::once(command.name.to_string()).chain(command.aliases.iter().map(|word| word.to_string()))).collect(),
    };

    let wiki = Arc::new(Wiki::open());

    tokio::spawn(keep_translated(Arc::clone(&wiki)));
    tokio::spawn(keep_fresh(config.source, shelf, wiki));

    let options = poise::FrameworkOptions {
        commands,
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

fn closing_line(content: &str, prefix: &str) -> Option<usize> {
    let body = content.trim_end();
    let line = body.rfind('\n').map_or(0, |at| at + 1);
    let text = body.get(line..)?.trim_start();
    let start = body.len() - text.len();

    text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)).then_some(start + prefix.len())
}

fn prefix_length(content: &str, prefix: &str, me: serenity::UserId, direct: bool, replied_to: Option<serenity::UserId>) -> Option<usize> {
    let text = content.trim_start();
    let leading = content.len() - text.len();

    if text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)) {
        return Some(leading + prefix.len());
    }

    if let Some(length) = closing_line(content, prefix) {
        return Some(length);
    }

    for mention in [format!("<@{me}>"), format!("<@!{me}>")] {
        if text.starts_with(mention.as_str()) {
            return Some(leading + mention.len());
        }
    }

    (direct || replied_to == Some(me)).then_some(leading)
}

fn unasked(prefix: &str) -> bool {
    prefix.trim().is_empty()
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

            let Some((prefix, text)) = msg.content.split_at_checked(usize::from(content_start)).filter(|(prefix, _)| !unasked(prefix)) else {
                return;
            };
            let voice = Voice::Message(ctx, msg, data);

            debug!(prefix, "A request came without a command");

            if let Err(err) = asked(voice, Scope::Any, text).await {
                error!("A request without a command failed: {}", err);

                if let Err(err) = voice.say(prose::FAILED.said(voice.prose())).await {
                    error!("The failure notice could not be sent: {}", err);
                }
            }
        }
        FrameworkError::CommandCheckFailed { .. } => {}
        FrameworkError::ArgumentParse { ctx: poise::Context::Prefix(typed), input, .. } if unasked(typed.prefix()) => {
            debug!(command = %typed.command().name, input, "Chat that only looked like a command was left alone");
        }
        other => {
            if let Err(err) = poise::builtins::on_error(other).await {
                error!("An error could not be handled: {}", err);
            }
        }
    }
}

async fn keep_translated(wiki: Arc<Wiki>) {
    loop {
        tokio::time::sleep(wiki.wait()).await;

        let held = Arc::clone(&wiki);
        let whole = task::spawn_blocking(move || held.refresh()).await.unwrap_or_else(|err| {
            error!("The wiki name refresh stopped: {}", err);
            false
        });

        if !whole {
            tokio::time::sleep(WIKI_RETRY).await;
        }
    }
}

async fn keep_fresh(source: String, shelf: Shelf, wiki: Arc<Wiki>) {
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
                let names = Arc::clone(&wiki);
                let loaded = task::spawn_blocking(move || {
                    library::download(&link).map(|payload| {
                        let catalog = Catalog::new(payload, names);

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
                    *shelf.write().await = Some(Arc::new(Catalog::new(String::new(), Arc::clone(&wiki))));
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

impl Posted<'_> {
    async fn slip(&self, voice: Voice<'_>) -> Option<(u64, Slip)> {
        match (self, voice) {
            (Posted::Command(handle), Voice::Command(ctx)) => match handle.message().await {
                Ok(message) => Some((
                    message.id.get(),
                    match ctx {
                        poise::Context::Application(asked) => Slip::Reply(asked.interaction.token.clone()),
                        poise::Context::Prefix(_) => Slip::Message(message.channel_id, message.id),
                    },
                )),
                Err(err) => {
                    warn!("A pick list could not be found again, so it will not time out: {}", err);
                    None
                }
            },
            (Posted::Press, Voice::Press(_, press, _)) => Some((press.message.id.get(), Slip::Reply(press.token.clone()))),
            (Posted::Message(sent), Voice::Message(_, msg, _)) => Some((sent.get(), Slip::Message(msg.channel_id, *sent))),
            _ => None,
        }
    }
}

fn waits(data: &Data) -> MutexGuard<'_, HashMap<u64, Wait>> {
    data.waits.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn expiring(voice: Voice<'_>, posted: &Posted<'_>) {
    let Some((message, slip)) = posted.slip(voice).await else {
        return;
    };
    let wait = Wait { due: Instant::now() + PICK_WAIT, slip, prose: voice.prose() };
    let fresh = waits(voice.data()).insert(message, wait).is_none();

    if fresh {
        tokio::spawn(lapse(Arc::clone(&voice.discord().http), Arc::clone(&voice.data().waits), message));
    }
}

fn extend(held: &mut HashMap<u64, Wait>, message: Option<serenity::MessageId>, token: &str) {
    if let Some(wait) = message.and_then(|message| held.get_mut(&message.get())) {
        wait.due = Instant::now() + PICK_WAIT;
        wait.slip = Slip::Reply(token.to_owned());
    }
}

fn touched(data: &Data, message: Option<serenity::MessageId>, token: &str) {
    extend(&mut waits(data), message, token);
}

async fn lapse(http: Arc<serenity::Http>, waits: Waits, message: u64) {
    loop {
        let due = waits.lock().unwrap_or_else(PoisonError::into_inner).get(&message).map(|wait| wait.due);
        let Some(due) = due else {
            return;
        };

        if Instant::now() < due {
            tokio::time::sleep_until(due.into()).await;
            continue;
        }

        let Some(wait) = waits.lock().unwrap_or_else(PoisonError::into_inner).remove(&message) else {
            return;
        };
        let text = prose::PICK_LAPSED.filled(wait.prose, &[("minutes", &(PICK_WAIT.as_secs() / 60).to_string())]);
        let edited = match wait.slip {
            Slip::Message(channel, sent) => channel.edit_message(&http, sent, serenity::EditMessage::new().content(text).components(Vec::new()).allowed_mentions(quiet())).await.map(drop),
            Slip::Reply(token) => serenity::EditInteractionResponse::new().content(text).components(Vec::new()).execute(&http, &token).await.map(drop),
        };

        if let Err(err) = edited {
            warn!(message, "A pick list could not be marked as timed out: {}", err);
        }

        return;
    }
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

    fn discord(self) -> &'a serenity::Context {
        match self {
            Voice::Command(ctx) => ctx.serenity_context(),
            Voice::Press(ctx, _, _) | Voice::Message(ctx, _, _) => ctx,
        }
    }

    fn guild(self) -> Option<serenity::GuildId> {
        match self {
            Voice::Command(ctx) => ctx.guild_id(),
            Voice::Press(_, press, _) => press.guild_id,
            Voice::Message(_, msg, _) => msg.guild_id,
        }
    }

    fn asker(self) -> Asker {
        self.guild().filter(|_| !self.personal()).map_or_else(|| Asker::User(self.user().get()), |guild| Asker::Server(guild.get()))
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

async fn offer(voice: Voice<'_>, prompt: String, labeled: bool, tails: &[(Kind, String)], leading: Vec<Choice>, found: Vec<Hit>) -> Result<(), Error> {
    offer_as(voice, prompt, labeled, None, tails, leading, found).await
}

fn turns(shown: usize, pages: usize, turn: impl Fn(String) -> String) -> Vec<serenity::CreateButton> {
    let button = |to: Option<usize>, side: &str, label: &str| serenity::CreateButton::new(turn(to.map_or_else(|| side.to_owned(), |to| to.to_string()))).label(label).style(serenity::ButtonStyle::Secondary).disabled(to.is_none());

    vec![
        button(shown.checked_sub(1), "first", "◀"),
        serenity::CreateButton::new(turn(format!("{JUMP}{shown}"))).label(format!("{} / {pages}", shown + 1)).style(serenity::ButtonStyle::Secondary).disabled(pages <= 1),
        button(Some(shown + 1).filter(|next| *next < pages), "last", "▶"),
    ]
}

fn pager(shown: usize, pages: usize, turn: impl Fn(String) -> String) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(turns(shown, pages, turn))
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

async fn offer_as(voice: Voice<'_>, prompt: String, labeled: bool, route: Option<&str>, tails: &[(Kind, String)], leading: Vec<Choice>, found: Vec<Hit>) -> Result<(), Error> {
    let room = (SEARCH_SHOWN * LIST_PAGES).saturating_sub(leading.len());
    let mut choices = leading;

    for hit in found.into_iter().take(room) {
        let tail = tails.iter().find(|(kind, _)| *kind == hit.kind).map_or("", |(_, tail)| tail.as_str());
        let value = match (route, hit.kind) {
            (Some(route), Kind::Map) => format!("{}{route}|{}|{tail}", Kind::Map.tag(), hit.value),
            (Some(route), _) => format!("{route}|{}|{tail}", hit.value),
            (None, kind) => format!("{}|{}|{tail}", kind.tag(), hit.value),
        };

        let note = if labeled { format!("{} • {}", hit.kind.label(), hit.note) } else { hit.note };

        choices.push((hit.label.chars().take(OPTION_LIMIT).collect::<String>(), value, note.chars().take(OPTION_LIMIT).collect::<String>()));
    }

    offer_choices(voice, prompt, choices).await
}

async fn offer_choices(voice: Voice<'_>, prompt: String, choices: Vec<Choice>) -> Result<(), Error> {
    let owner = voice.user().get();

    if choices.iter().any(|(_, value, _)| value.len() > VALUE_LIMIT) {
        return voice.say(prose::SEARCH_LONG.said(voice.prose())).await;
    }

    let choices: Arc<[Choice]> = choices.into();
    let token = (choices.len() > SEARCH_SHOWN).then(|| lists(voice.data()).keep(owner, Arc::clone(&choices)));
    let posted = voice.post(Post::text(prompt).rows(list_rows(owner, token, &choices, 0))).await?;

    expiring(voice, &posted).await;

    Ok(())
}

async fn on_seek(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(3, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let token = parts.next().and_then(|token| u64::from_str_radix(token, 16).ok());
    let (Some(owner), Some(token), Some(slot)) = (owner, token, parts.next()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    touched(data, Some(press.message.id), &press.token);

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
        Ok(post) => {
            let picking = !post.rows.is_empty();
            let posted = voice.post(post).await?;

            if picking {
                expiring(voice, &posted).await;
            }

            Ok(())
        }
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
    let Some((_, code)) = sought(voice, &library, &prefs, Scope::Only(Kind::Map), &request, &prose::NO_MAP).await? else { return Ok(()) };
    let crowned = request.crowns.map(|crowns| format!("-c {crowns}"));
    let moded = request.mode().map(|mode| format!("--{}", mode.slug()));
    let tail = crowned.into_iter().chain(moded).collect::<Vec<_>>().join(" ");

    listed(voice, &library, &code, Kind::Stage.tag(), &tail).await
}

async fn on_chart(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.splitn(5, '|');
    let owner = parts.next().and_then(|owner| owner.parse::<u64>().ok());
    let (Some(owner), Some(route), Some(code), Some(slot), Some(tail)) = (owner, parts.next(), parts.next(), parts.next(), parts.next()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press, data).await;
    }

    touched(data, Some(press.message.id), &press.token);

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

async fn send_stage(voice: Voice<'_>, library: Arc<Library>, code: String, asked: Option<u8>, mode: Mode) -> Result<(), Error> {
    let prose = voice.prose();
    let Some(found) = library.stage(&code) else {
        return voice.say(prose::NO_STAGE.said(prose)).await;
    };
    let crown = match stage::crown(found.stage, asked) {
        Ok(crown) => crown,
        Err(trouble) => return voice.say(trouble.told(prose)).await,
    };
    let rows = if mode == Mode::Interactive { crown_rows(voice.user().get(), &code, found.stage, crown) } else { Vec::new() };
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

async fn sought(voice: Voice<'_>, library: &Library, prefs: &Prefs, scope: Scope, request: &Request, missing: &Line) -> Result<Option<(Kind, String)>, Error> {
    let kinds = scope.kinds();
    let asked = match &request.unit {
        Unit::Name(name) | Unit::Code(name) => name.clone(),
        Unit::Id(id, _) => id.to_string(),
    };
    let mut found = lookup::among(library, kinds, &request.unit, request.search.unwrap_or(fuzzy::CLOSENESS), prefs);
    let best = found.first().map(|hit| hit.score);
    let leading = request.search.map_or_else(Vec::new, |closeness| alias_choices(library, prefs, scope, &asked, closeness, &request.tail));

    if request.search.is_none() {
        found.retain(|hit| Some(hit.score) == best);
    }

    match (request.search.is_some(), found.as_slice()) {
        (_, []) if leading.is_empty() => voice.say(missing.said(prefs.prose)).await.map(|()| None),
        (false, [only]) => Ok(Some((only.kind, only.value.clone()))),
        (searched, _) => {
            let line = if searched { &prose::SEARCH_PICK } else { &prose::EITHER_PICK };
            let tails: Vec<(Kind, String)> = kinds.iter().map(|kind| (*kind, request.tail.clone())).collect();

            offer_as(voice, line.filled(prefs.prose, &[("name", &asked)]), kinds.len() > 1, scope.route(), &tails, leading, found).await.map(|()| None)
        }
    }
}

async fn serve_stage(voice: Voice<'_>, request: Request) -> Result<(), Error> {
    let prefs = voice.data().book.get(voice.user().get());

    voice.working().await?;

    let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
        return voice.say(prose::WARMING.said(prefs.prose)).await;
    };
    let Some((_, code)) = sought(voice, &library, &prefs, Scope::Only(Kind::Stage), &request, &prose::NO_STAGE).await? else { return Ok(()) };

    send_stage(voice, Arc::clone(&library), code, request.crowns.map(|crowns| crowns.saturating_sub(1)), request.mode().unwrap_or(prefs.mode)).await
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

    send_stage(voice, library, code.to_owned(), Some(crown), Mode::Interactive).await
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scope {
    Any,
    Only(Kind),
    Theme,
    Backdrop,
}

impl Scope {
    fn tag(self) -> &'static str {
        match self {
            Scope::Any => "",
            Scope::Only(kind) => kind.tag(),
            Scope::Theme => THEME,
            Scope::Backdrop => BACKDROP,
        }
    }

    fn tagged(tag: &str) -> Option<Self> {
        match tag {
            "" => Some(Scope::Any),
            THEME => Some(Scope::Theme),
            BACKDROP => Some(Scope::Backdrop),
            _ => Kind::ALL.into_iter().find(|kind| kind.tag() == tag).map(Scope::Only),
        }
    }

    fn kinds(&self) -> &[Kind] {
        match self {
            Scope::Any => &Kind::ALL,
            Scope::Only(kind) => std::slice::from_ref(kind),
            Scope::Theme | Scope::Backdrop => &Kind::PLACES,
        }
    }

    fn route(self) -> Option<&'static str> {
        match self {
            Scope::Theme => Some(THEME),
            Scope::Backdrop => Some(BACKDROP),
            Scope::Any | Scope::Only(_) => None,
        }
    }
}

fn rerouted(aliases: &Aliases, scope: Scope, text: &str) -> Option<(Scope, String)> {
    match scope {
        Scope::Any => Kind::ALL
            .into_iter()
            .find_map(|kind| grammar::parse(text, kind).ok().and_then(|request| aliases.reroute(&request, &[kind], false)))
            .map(|(kind, text)| (kind.map_or(Scope::Any, Scope::Only), text)),
        Scope::Only(kind) => grammar::parse(text, kind).ok().and_then(|request| aliases.reroute(&request, &[kind], true)).map(|(_, text)| (scope, text)),
        Scope::Theme => match grammar::tune(text).ok()? {
            Tune::Place(request, boss) => aliases.reroute(&request, &Kind::PLACES, true).map(|(_, text)| (scope, if boss { format!("{text} --{BOSS}") } else { text })),
            Tune::Track(..) => None,
        },
        Scope::Backdrop => match grammar::scenery(text).ok()?.subject {
            Backing::Place(request) => aliases.reroute(&request, &Kind::PLACES, true).map(|(_, text)| (scope, text)),
            Backing::Id(..) => None,
        },
    }
}

fn chosen(aliases: &Aliases, scope: Scope, key: &str, tail: &str) -> Option<(Scope, String)> {
    let (kind, target) = match aliases.keyed(key)? {
        Target::Typed(target) => (None, target),
        Target::Known(kind, unit) | Target::Original(kind, unit) => (Some(kind), unit),
    };
    let text = match scope {
        Scope::Theme if tail == BOSS => format!("{target} --{BOSS}"),
        Scope::Theme => target.to_owned(),
        _ if tail.is_empty() => target.to_owned(),
        _ => format!("{target} {tail}"),
    };

    Some((kind.filter(|_| scope == Scope::Any).map_or(scope, Scope::Only), text))
}

fn clipped(text: &str) -> String {
    text.chars().take(OPTION_LIMIT).collect()
}

fn told(library: Option<&Library>, prefs: &Prefs, target: Target<'_>) -> String {
    match target {
        Target::Typed(typed) => typed.to_owned(),
        Target::Known(kind, unit) | Target::Original(kind, unit) => {
            let named = library.and_then(|library| grammar::parse(unit, kind).ok().and_then(|request| lookup::hits(library, kind, &request.unit, fuzzy::CLOSENESS, prefs).into_iter().next())).map(|hit| hit.label);

            named.unwrap_or_else(|| format!("{} {unit}", kind.label()))
        }
    }
}

fn alias_choices(library: &Library, prefs: &Prefs, scope: Scope, asked: &str, closeness: u32, tail: &str) -> Vec<Choice> {
    prefs
        .aliases
        .close(asked, closeness, scope.kinds())
        .into_iter()
        .map(|row| (clipped(row.name), format!("{ALIAS}{}|{}|{tail}", scope.tag(), row.key), clipped(&format!("Alias • {}", told(Some(library), prefs, row.target)))))
        .filter(|(_, value, _)| value.len() <= VALUE_LIMIT)
        .collect()
}

async fn asked(voice: Voice<'_>, scope: Scope, text: &str) -> Result<(), Error> {
    let aliases = voice.data().book.aliases(voice.user().get());

    match rerouted(&aliases, scope, text) {
        Some((scope, text)) => answer(voice, scope, &text).await,
        None => answer(voice, scope, text).await,
    }
}

async fn answer(voice: Voice<'_>, scope: Scope, text: &str) -> Result<(), Error> {
    let unread = match scope {
        Scope::Any => return serve_any(voice, text).await,
        Scope::Only(kind) => match grammar::parse(text, kind) {
            Ok(request) => return serve(voice, kind, request).await,
            Err(reason) => reason,
        },
        Scope::Theme => match grammar::tune(text) {
            Ok(Tune::Track(id, typed)) => return serve_audio(voice, id, typed).await,
            Ok(Tune::Place(request, boss)) => return serve_theme(voice, request, boss, &prose::NO_PLACE).await,
            Err(reason) => reason,
        },
        Scope::Backdrop => match grammar::scenery(text) {
            Ok(scenery) => return serve_background(voice, scenery).await,
            Err(reason) => reason,
        },
    };

    voice.say(prose::SEE_HELP.filled(voice.prose(), &[("reason", &unread), ("prefix", &prefix_of(voice.data(), voice.user().get()))])).await
}

async fn serve_any(voice: Voice<'_>, text: &str) -> Result<(), Error> {
    let owner = voice.user().get();
    let prefs = voice.data().book.get(owner);
    let (found, wide) = {
        let Some((_guard, library)) = open(voice.data(), &prefs.language).await? else {
            return voice.say(prose::WARMING.said(prefs.prose)).await;
        };
        let wide = lookup::searching(text).map(|(asked, closeness, tail)| (alias_choices(&library, &prefs, Scope::Any, &asked, closeness, &tail), asked)).filter(|(rows, _)| !rows.is_empty());

        (lookup::across(&library, &prefs, text), wide)
    };

    match (found, wide) {
        (Found::Unread(_) | Found::Nothing, Some((rows, asked))) => offer_choices(voice, prose::SEARCH_PICK.filled(prefs.prose, &[("name", &asked)]), rows).await,
        (Found::Unread(reason), None) => voice.say(prose::SEE_HELP.filled(prefs.prose, &[("reason", &reason), ("prefix", &prefix_of(voice.data(), owner))])).await,
        (Found::Nothing, None) => voice.say(prose::NO_UNIT.filled(prefs.prose, &[("word", text.split_whitespace().next().unwrap_or_default()), ("prefix", &prefix_of(voice.data(), owner))])).await,
        (Found::One(kind, request), _) => match prefs.aliases.original(&request, kind).map(|text| grammar::parse(&text, kind)) {
            Some(Ok(original)) => serve(voice, kind, original).await,
            Some(Err(reason)) => voice.say(prose::SEE_HELP.filled(prefs.prose, &[("reason", &reason), ("prefix", &prefix_of(voice.data(), owner))])).await,
            None => serve(voice, kind, request).await,
        },
        (Found::Several { asked, searched, tails, hits }, wide) => {
            let line = if searched { &prose::SEARCH_PICK } else { &prose::EITHER_PICK };

            offer(voice, line.filled(prefs.prose, &[("name", &asked)]), true, &tails, wide.map_or_else(Vec::new, |(rows, _)| rows), hits).await
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

    waits(data).remove(&press.message.id.get());
    press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    if picked == ABORT {
        return voice.say(prose::SEARCH_ABORT.said(voice.prose())).await;
    }

    let mut parts = picked.splitn(3, '|');
    let (Some(tag), Some(unit), Some(tail)) = (parts.next(), parts.next(), parts.next()) else { return Ok(()) };
    let kind = Kind::ALL.into_iter().find(|kind| kind.tag() == tag);
    let mapped = tag.strip_prefix(Kind::Map.tag()).filter(|route| [THEME, BACKDROP].contains(route));
    let aliased = tag.strip_prefix(ALIAS).and_then(Scope::tagged).map(|scope| chosen(&data.book.aliases(owner), scope, unit, tail));
    let served = match (aliased, tag, mapped, kind.map(|kind| (kind, grammar::parse(&format!("{unit} {tail}"), kind)))) {
        (Some(Some((scope, text))), ..) => answer(voice, scope, &text).await,
        (Some(None), ..) => voice.say(prose::ALIAS_GONE.said(voice.prose())).await,
        (None, _, Some(route), _) => charted(voice, data, owner, unit, route, tail).await,
        (None, THEME, ..) => themed(voice, data, owner, unit, tail == BOSS).await,
        (None, BACKDROP, ..) => backed(voice, data, owner, unit, tail).await,
        (None, _, _, Some((kind, Ok(request)))) => serve(voice, kind, request).await,
        (None, _, _, Some((_, Err(reason)))) => voice.say(prose::SEE_HELP.filled(voice.prose(), &[("reason", &reason), ("prefix", &prefix_of(data, owner))])).await,
        (None, _, _, None) => return Ok(()),
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
    page: Page,
    versus: Versus,
    lens: Lens,
}

impl Sheet {
    fn custom(&self, tag: &str, form: usize) -> String {
        self.turned(tag, form, self.page)
    }

    fn turned(&self, tag: &str, form: usize, page: Page) -> String {
        format!("{tag}|{}|{}|{}|{}|{}|{}|{:x}|{}", self.owner, self.id, form, self.level.as_deref().unwrap_or_default(), self.talents.as_deref().unwrap_or("-"), page.tag(), self.versus.bits(), grammar::lens_tag(self.lens))
    }

    fn read(custom: &str) -> Option<(&str, Self)> {
        let mut parts = custom.split('|');
        let tag = parts.next()?;
        let owner = parts.next()?.parse().ok()?;
        let id = parts.next()?.parse().ok()?;
        let form = parts.next()?.parse().ok()?;
        let level = Some(parts.next()?).filter(|level| !level.is_empty()).map(str::to_owned);
        let talents = parts.next().filter(|levels| *levels != "-").map(str::to_owned);
        let page = parts.next().and_then(Page::tagged).unwrap_or_default();
        let versus = parts.next().and_then(|bits| u32::from_str_radix(bits, 16).ok()).map_or_else(Versus::default, Versus::from_bits);
        let lens = parts.next().map_or_else(Lens::default, grammar::lens_tagged);

        Some((tag, Self { owner, id, form, level, talents, page, versus, lens }))
    }
}

#[derive(Clone)]
struct Card {
    owner: u64,
    id: u32,
    page: Page,
    magnification: Option<(i32, i32)>,
    lens: Lens,
}

impl Card {
    const TAGS: [&'static str; 3] = [CARD_PAGE, CARD_ASK, CARD_SET];

    fn turned(&self, tag: &str, page: Page) -> String {
        format!("{tag}|{}|{}|{}|{}|{}", self.owner, self.id, page.tag(), self.magnification.map(percents).unwrap_or_default(), grammar::lens_tag(self.lens))
    }

    fn read(custom: &str) -> Option<(&str, Self)> {
        let mut parts = custom.split('|');
        let tag = parts.next().filter(|tag| Self::TAGS.contains(tag))?;
        let owner = parts.next()?.parse().ok()?;
        let id = parts.next()?.parse().ok()?;
        let page = Page::tagged(parts.next()?)?;
        let magnification = parts.next().and_then(grammar::magnification_of);
        let lens = parts.next().map_or_else(Lens::default, grammar::lens_tagged);

        Some((tag, Self { owner, id, page, magnification, lens }))
    }
}

fn percents((hitpoints, attack): (i32, i32)) -> String {
    if hitpoints == attack { format!("{hitpoints}%") } else { format!("{hitpoints}%/{attack}%") }
}

fn talented(cat: &CatEntry, form: usize) -> bool {
    cat.talent_data.is_some() && form >= TRUE_FORM
}

fn sheet_png(library: &Library, cache: &Cache, sheet: &Sheet) -> Result<(Vec<u8>, String), Trouble> {
    let cat = library.cat(sheet.id).ok_or(Trouble::NoCat)?;
    let id = cat.id_str(sheet.form);

    if sheet.page == Page::Details {
        return Ok((statblock::cat_details(library, cache, cat, sheet.form)?, format!("{id}.details.png")));
    }

    let caps = cat.unitbuy.level_caps();
    let levels = sheet.talents.as_deref().and_then(talents::decoded);
    let shown = sheet.level.as_deref().map_or_else(|| library.level_for(cat, sheet.form, levels.as_ref()), |level| level::capped(level, caps));
    let total = level::total(&shown, caps);

    let asked = statblock::Shown { cat, form: sheet.form, level: total, level_input: &shown, talents: levels.as_ref() };

    match sheet.page {
        Page::Talents => Ok((statblock::talents(library, cache, asked)?, format!("{id}.talents.png"))),
        _ => Ok((statblock::cat(library, cache, asked, sheet.versus, sheet.lens)?, format!("{id}.png"))),
    }
}

fn card_png(library: &Library, cache: &Cache, card: &Card) -> Result<(Vec<u8>, String), Trouble> {
    let enemy = library.enemy(card.id).ok_or(Trouble::NoEnemy)?;
    let id = enemy.id_str();

    match card.page {
        Page::Details => Ok((statblock::enemy_details(library, cache, enemy)?, format!("{id}.details.png"))),
        _ => {
            let plain = Magnification::default();
            let (hitpoints, attack) = card.magnification.unwrap_or((plain.hitpoints, plain.attack));

            Ok((statblock::enemy(library, cache, enemy, Magnification::pair(hitpoints, attack), card.lens)?, format!("{id}.png")))
        }
    }
}

fn lit(on: bool) -> serenity::ButtonStyle {
    if on { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary }
}

fn card_rows(card: &Card) -> Vec<serenity::CreateActionRow> {
    let turn = |page: Page, label: &str| pick(card.page == page, card.turned(CARD_PAGE, page), label);
    let pages = vec![caption(format!("{LABEL}|0"), "Page"), turn(Page::Stats, "Stats"), turn(Page::Details, "Details")];
    let tuning = vec![caption(format!("{LABEL}|1"), "Apply"), serenity::CreateButton::new(card.turned(CARD_ASK, card.page)).label("Magnification").style(lit(card.magnification.is_some()))];

    vec![serenity::CreateActionRow::Buttons(pages), serenity::CreateActionRow::Buttons(tuning)]
}

fn sheet_rows(library: &Library, sheet: &Sheet) -> Vec<serenity::CreateActionRow> {
    let Some(cat) = library.cat(sheet.id) else {
        return Vec::new();
    };
    let mut forms = vec![caption(format!("{LABEL}|0"), "Form")];

    forms.extend((0..FORMS.len()).map(|form| {
        let page = if sheet.page == Page::Talents && !talented(cat, form) { Page::Stats } else { sheet.page };

        pick(form == sheet.form, sheet.turned("sb", form, page), FORMS[form]).disabled(!cat.forms[form])
    }));

    let turn = |page: Page, label: &str| pick(sheet.page == page, sheet.turned("sp", sheet.form, page), label);
    let gifted = talented(cat, sheet.form);
    let pages = vec![caption(format!("{LABEL}|1"), "Page"), turn(Page::Stats, "Stats"), turn(Page::Talents, "Talents").disabled(!gifted), turn(Page::Details, "Details")];
    let tuning = vec![
        caption(format!("{LABEL}|2"), "Apply"),
        serenity::CreateButton::new(sheet.custom("sl", sheet.form)).label("Level").style(lit(sheet.level.is_some())),
        serenity::CreateButton::new(sheet.custom("st", sheet.form)).label("Talents").style(lit(sheet.talents.is_some())).disabled(!gifted),
        serenity::CreateButton::new(sheet.custom("sx", sheet.form)).label("Versus").style(lit(!sheet.versus.is_empty())),
    ];

    vec![serenity::CreateActionRow::Buttons(forms), serenity::CreateActionRow::Buttons(pages), serenity::CreateActionRow::Buttons(tuning)]
}

async fn send_sheet(voice: Voice<'_>, library: Arc<Library>, sheet: Sheet, mode: Mode) -> Result<(), Error> {
    let rendered = {
        let (library, sheet) = (Arc::clone(&library), sheet.clone());

        let cache = Arc::clone(&voice.data().cache);

        blocking(voice.data(), move || sheet_png(&library, &cache, &sheet)).await?
    };

    match rendered {
        Ok((bytes, name)) => voice.post(Post::file(bytes, name).rows(if mode == Mode::Interactive { sheet_rows(&library, &sheet) } else { Vec::new() })).await.map(|_| ())?,
        Err(trouble) => voice.say(trouble.told(voice.prose())).await?,
    };

    Ok(())
}

async fn send_card(voice: Voice<'_>, library: Arc<Library>, card: Card, mode: Mode) -> Result<(), Error> {
    let rendered = {
        let (library, card, cache) = (Arc::clone(&library), card.clone(), Arc::clone(&voice.data().cache));

        blocking(voice.data(), move || card_png(&library, &cache, &card)).await?
    };

    match rendered {
        Ok((bytes, name)) => voice.post(Post::file(bytes, name).rows(if mode == Mode::Interactive { card_rows(&card) } else { Vec::new() })).await.map(|_| ())?,
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
            done = &mut work => break done,
            () = tokio::time::sleep(TICK) => {
                if let Err(err) = voice.amend(&posted, Post::text(progress_bar(&label, searching, &progress, started.elapsed()))).await {
                    warn!("The progress bar could not be updated: {}", err);
                }
            }
        }
    };

    tally(voice, started).await;

    match finished? {
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

async fn tally(voice: Voice<'_>, started: Instant) {
    let (asker, ended) = (voice.asker(), Instant::now());
    let Tally { sessions, busy, alert } = voice.data().usage.lock().unwrap_or_else(PoisonError::into_inner).note(asker, started, ended);

    info!(asker = asker.name(), id = asker.id(), ms = ended.saturating_duration_since(started).as_millis(), sessions, busy_seconds = busy.as_secs(), "An encoding session ended");

    let Some(alert) = alert else {
        return;
    };

    warn!(asker = asker.name(), id = asker.id(), "{}", alert);

    if let Err(err) = serenity::ChannelId::new(usage::ALERT_CHANNEL).send_message(voice.discord(), serenity::CreateMessage::new().content(alert).allowed_mentions(quiet())).await {
        error!(asker = asker.name(), id = asker.id(), "The abuse alert could not be sent: {}", err);
    }
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
            let Some((kind, code)) = sought(voice, &library, &prefs, Scope::Backdrop, request, missing).await? else { return Ok(()) };

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
    let Some((kind, code)) = sought(voice, &library, &prefs, Scope::Theme, &request, missing).await? else { return Ok(()) };

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

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("c"))]
async fn cat(ctx: Context<'_>, #[description = "ID-form or name, then stats or anim"] #[rest] request: Option<String>) -> Result<(), Error> {
    asked(Voice::Command(ctx), Scope::Only(Kind::Cat), request.as_deref().unwrap_or_default()).await
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
        let leading = alias_choices(&library, &prefs, Scope::Only(Kind::Cat), name, closeness, &request.tail);

        if found.is_empty() && leading.is_empty() {
            return voice.say(prose::NO_CAT.said(prefs.prose)).await;
        }

        return offer(voice, prose::SEARCH_PICK.filled(prefs.prose, &[("name", name)]), false, &[(Kind::Cat, request.tail)], leading, found).await;
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
        Want::Stats(stats) => {
            let words: Vec<&str> = stats.versus.iter().flatten().map(String::as_str).collect();
            let versus = match grammar::versus_of(&words, &foes(&library, &prefs.aliases)) {
                Ok(versus) => versus,
                Err(word) => return voice.say(prose::NOT_A_TRAIT.filled(prefs.prose, &[("name", &word)])).await,
            };
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
            let lifted = talents.is_some() || stats.page == Page::Talents;
            let form = if lifted && form < TRUE_FORM && cat.forms[TRUE_FORM] { TRUE_FORM } else { form };

            if stats.page == Page::Talents && !talented(cat, form) {
                voice.say(Trouble::NoTalents(cat.display_name(form)).told(prefs.prose)).await?;
                return Ok(());
            }

            let lens = Lens { mode: stats.gauge.unwrap_or_default(), hits: stats.hits.unwrap_or_default() };
            let sheet = Sheet { owner, id: cat.id, form, level: stats.level, talents, page: stats.page, versus, lens };

            send_sheet(voice, Arc::clone(&library), sheet, stats.mode.unwrap_or(prefs.mode)).await
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
    asked(Voice::Command(ctx), Scope::Only(Kind::Enemy), request.as_deref().unwrap_or_default()).await
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
        let leading = alias_choices(&library, &prefs, Scope::Only(Kind::Enemy), name, closeness, &request.tail);

        if found.is_empty() && leading.is_empty() {
            return voice.say(prose::NO_ENEMY.said(prefs.prose)).await;
        }

        return offer(voice, prose::SEARCH_PICK.filled(prefs.prose, &[("name", name)]), false, &[(Kind::Enemy, request.tail)], leading, found).await;
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
            let lens = Lens { mode: stats.gauge.unwrap_or_default(), hits: stats.hits.unwrap_or_default() };
            let card = Card { owner: voice.user().get(), id: enemy.id, page: stats.page, magnification: stats.magnification, lens };

            send_card(voice, Arc::clone(&library), card, stats.mode.unwrap_or(prefs.mode)).await
        }
        Want::Motion(motion) => {
            let set = enemy_animation::motions(enemy, &library.vault.vfs);
            let title = enemy.display_name();

            animate(voice, Arc::clone(&library), set, options_of(&motion, &prefs), title).await
        }
    }
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("s"))]
async fn stage(ctx: Context<'_>, #[description = "A stage's name or ID like R 1 2, then --crowns 1 to 4"] #[rest] request: Option<String>) -> Result<(), Error> {
    asked(Voice::Command(ctx), Scope::Only(Kind::Stage), request.as_deref().unwrap_or_default()).await
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("m"))]
async fn map(ctx: Context<'_>, #[description = "A map's name or ID like R 100, then --crowns 1 to 4"] #[rest] request: Option<String>) -> Result<(), Error> {
    asked(Voice::Command(ctx), Scope::Only(Kind::Map), request.as_deref().unwrap_or_default()).await
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("bg", "b"))]
async fn background(ctx: Context<'_>, #[description = "ID or stage name, then --seed, --range, --offset, --dimensions or --format"] #[rest] request: Option<String>) -> Result<(), Error> {
    asked(Voice::Command(ctx), Scope::Backdrop, request.as_deref().unwrap_or_default()).await
}

#[poise::command(slash_command, prefix_command, install_context = "Guild|User", interaction_context = "Guild|BotDm|PrivateChannel", aliases("a"))]
async fn audio(ctx: Context<'_>, #[description = "A track's ID like 5, or a stage's name, then --boss"] #[rest] id: Option<String>) -> Result<(), Error> {
    asked(Voice::Command(ctx), Scope::Theme, id.as_deref().unwrap_or_default()).await
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
const HELP: &str = "help|";
const GENERAL: &str = "general";
const UNIT_SECTIONS: [(&str, &str); 5] = [(GENERAL, "General"), ("unit", "Unit"), ("stats", "Stats"), ("animation", "Animation"), ("buttons", "Buttons")];

fn sections(page: &str) -> &'static [(&'static str, &'static str)] {
    match page {
        "cat" | "enemy" => &UNIT_SECTIONS,
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
    let menu = |custom: String, held: &[(&str, &str)], shown: &str, placeholder: &str| {
        let options = held.iter().map(|(key, label)| serenity::CreateSelectMenuOption::new(*label, *key).default_selection(*key == shown)).collect();

        serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(custom, serenity::CreateSelectMenuKind::String { options }).placeholder(placeholder))
    };
    let held = sections(current);
    let parts = (!held.is_empty()).then(|| menu(format!("{HELP}{owner}|{current}"), held, section, "Sections"));

    std::iter::once(menu(format!("{HELP}{owner}|"), &PAGES, current, "Commands")).chain(parts).collect()
}

fn page(title: &str, description: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new().title(title).description(description).color(EMBED_COLOR)
}

const CLIPS: &str = "`attack` (`atk`, `a`), `idle` (`i`), `walk` (`w`), `knockback` (`kb`, `k`), `showcase` (`sh`, `sc`), `burrow` (`bur`, `b`), `surface` (`su`, `sf`, `rise`, `r`), `dig` (`d`), `spirit` (`sp`, `c`, `conjure`), `model` (`m`) for a single picture of the unit with no animation. Attack if none is given.";
const MOTION_FLAGS: &str = "`--range 0~30` or `--range 0 30` (`-r`): frames to export, wrapping around the loop, and one number alone is a single frame; not for showcases or models
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

const NAMES: &str = "Spaces are fine, and close spellings still match. One the game has no English name for also answers to the name the Battle Cats Wiki gives it";
const SEARCHING: &str = "After a name: lists the closest matches for you to pick from, 24 to a page. ◀ and ▶ under the list turn its pages, and the page number between them opens a box to jump to one. A number after it, 1 to 100, is how close a name has to be: 86 to start, lower finds more";

fn help_page(page_name: &str, section: &str, prefix: &str, prose: Prose) -> serenity::CreateEmbed {
    let voiced = match (page_name, section) {
        ("cat", "unit") => Voiced::new(prefix, "hg!cat (c) • Unit", "What goes where `<unit>` is in `hg!cat <unit> ...`")
            .field("ID", "`25-3`, `25_t` or `25_s`: an ID and a form (1-4, n t u, or f c s u)\n`25` or `025`: your default form, or the next one down if it has none\nAn ID ending in `-e` is an enemy's, so Evolved is `-2` or `-c`", false)
            .field("Name", NAMES, false)
            .field("`--search` (`-s`)", SEARCHING, false)
            .field("`form` (`f`)", "After an ID or a name, picks a form: 1-4, n e t u, f c s u, or Normal, Evolved, True, Ultra", false),
        ("cat", "stats") => Voiced::new(prefix, "hg!cat (c) • Stats", "`hg!cat <unit> [s | t | d] ...`")
            .field("Pages", "`stats` (`s`, `stat`): the statblock, and what you get without a page\n`talents` (`t`, `talent`): every talent with its description, its level and what it changes. True and Ultra forms only, so a Normal or Evolved form is lifted to True. What follows it is read as talents, the same as after `--talents`: `t all`, `t 10 10 5`, `t attack buff 5`\n`details` (`d`, `detail`, `description`): the description, what evolving costs, and its cat combos\nA level and `--talents` go with any of them", false)
            .field("A level after `talents`", "A plain number there is a talent's level, so `t 30` sets the first talent to 30. The cat's own level keeps its mark, as everywhere: `t lv30`, `t level 30` or `t 50+70`.", false)
            .field("Level", "`lv50`, `lvl50`, `l50`, `level 50` or `50+70`. A plain `50` isn't read as a level, so give it its mark. Without one, a typical level for its rarity, raised to the level its form or its ultra talents unlock at if that's higher.", false)
            .field("`--talents` (`-t`)", "`all` (`a`), `standard` (`s`), `ultra` (`u`), `none` (`n`), levels in order like `10 10 5`, or names like `Attack Buff 5`, written the way the talents page shows them. Talents lift a Normal or Evolved form to True. The talents page (`t`) lists every one with its max level.", false)
            .field("`--versus` (`-v`)", "Trait names like `-v red` or `-v black metal`, or an enemy's name like `-v teacher bear` for all of its traits: the stats against one enemy with every one of them, by the game's own math. `targets` is every trait the cat has something for, `traits` every trait, `subtraits` every subtrait and `all` both, Metal aside in each; `none` clears them.\nAttack, DPS and Hitpoints change where the cat has something against it, and Hitpoints become the damage it takes to kill it. Against Metal a hit that isn't critical deals 1. A Sage shortens a cat's crowd control and its knockbacks unless the cat has Sage Slayer.\nOutlines: green does something, red does nothing or holds the cat back, orange is a picked trait the cat has nothing for. Half red, half green works only partly, and half green, half orange helps without being the whole story.", false)
            .field("`--mode` (`-m`), `--hits` (`-h`)", MODE_HELP, false)
            .field("Uptime", "Freeze, Slow, Weaken and Curse say how much of the time they keep a target under. The low end is the hits alone with the shortest time, since some enemies shrug off waves, surges and explosions; the high end adds them with the longest time. With `--versus` the time is that enemy's. A cat that dies after attacking goes by its recharge.", false)
            .field("`--interactive` (`-i`), `--dormant` (`-d`)", "With or without the buttons under the picture this time, whatever your Blocks setting says.", false),
        ("cat", "animation") => Voiced::new(prefix, "hg!cat (c) • Animation", "`hg!cat <unit> [a | anim] [type] ...`").field("Types", CLIPS, false).field("Flags", MOTION_FLAGS, false),
        ("cat", "buttons") => Voiced::new(prefix, "hg!cat (c) • Buttons", "With your Blocks setting on Interactive, or `--interactive` (`-i`) after a request, three rows sit under the picture. Form picks a form, Page turns between Stats, Talents and Details, and Apply sets a Level, Talents or the traits to show the stats Versus. The one you're on is green, and so are Level, Talents and Versus once you've set them. A form the cat doesn't have is grayed out, and so is Talents below the True form or on a cat without any. Only the person who asked can press them."),
        ("cat", _) => Voiced::new(prefix, "hg!cat (c)", "`hg!cat <unit> [stats | animation] ...`")
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a level or `--talents` means stats. `s` or `a` still decides outright, and stats is the default. `t` and `d` open the Talents and Details pages, so the dig animation is `dig` or `a d`.", false)
            .field("Examples", "`hg!c 137_u lv50+70 -t all`\n`hg!c 44 t 10 5 lv40`\n`hg!c bahamut cat d`\n`hg!c bahamut cat form normal`\n`hg!c jamiera cat a showcase -f webp`\n`hg!c 008 a walk -l -r 10~120`", false),
        ("enemy", "unit") => Voiced::new(prefix, "hg!enemy (e) • Unit", "What goes where `<unit>` is in `hg!enemy <unit> ...`")
            .field("ID", "`763` or `0763`, and `763-e` or `763_E` the way its statblock writes it. Without a command, the `e` is what says enemy and not cat", false)
            .field("Name", NAMES, false)
            .field("`--search` (`-s`)", SEARCHING, false),
        ("enemy", "stats") => Voiced::new(prefix, "hg!enemy (e) • Stats", "`hg!enemy <unit> [s | d] ...`")
            .field("Pages", "`stats` (`s`, `stat`): the statblock, and what you get without a page\n`details` (`d`, `detail`, `description`): its description", false)
            .field("Magnification", "`400%`, or `400%/200%` for hitpoints and attack. 100% without one.", false)
            .field("`--mode` (`-m`), `--hits` (`-h`)", MODE_HELP, false)
            .field("`--interactive` (`-i`), `--dormant` (`-d`)", "With or without the buttons under the picture this time, whatever your Blocks setting says.", false),
        ("enemy", "animation") => Voiced::new(prefix, "hg!enemy (e) • Animation", "`hg!enemy <unit> [a | anim] [type] ...`").field("Types", CLIPS, false).field("Flags", MOTION_FLAGS, false),
        ("enemy", "buttons") => Voiced::new(prefix, "hg!enemy (e) • Buttons", "With your Blocks setting on Interactive, or `--interactive` (`-i`) after a request, two rows sit under the picture. Page turns between Stats and Details, and Apply sets a Magnification. The page you're on is green, and so is Magnification once you've set one. Only the person who asked can press them."),
        ("enemy", _) => Voiced::new(prefix, "hg!enemy (e)", "`hg!enemy <unit> [stats | animation] ...`")
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a percentage means stats. `s` or `a` still decides outright, and stats is the default. `d` opens the Details page, so the dig animation is `dig` or `a d`.", false)
            .field("Examples", "`hg!e akaza 400%`, `hg!e akaza s 400`\n`hg!e akaza d`\n`hg!e 763 a attack -c -w`", false),
        ("stage", _) => Voiced::new(prefix, "hg!stage (s)", "`hg!stage <name> ...`")
            .field("What you get", prose::PAGE_STAGE.said(prose), false)
            .field("Name", "Spaces are fine, and close spellings still match. A name several stages share gets a list to pick from.\n`--search` (`-s`) after it lists the closest matches, 24 to a page, and a number after that, 1 to 100, is how close a name has to be", false)
            .field("ID", "A category's prefix, the map and the stage: `R 001 02`, `R-1-2`, `r001_02` and `R1 2` are all the same stage. A file's prefix works too, so `RN 0 7` is `N 0 7` and `EX 70 9` is `RE 70 9`.", false)
            .field("`--crowns 2` (`-c`)", "1 to 4, and the lowest one the stage is open at without it. A stage that doesn't have the one you ask for says so.", false)
            .field("`--interactive` (`-i`), `--dormant` (`-d`)", "With or without the buttons under the picture this time, whatever your Blocks setting says.", false)
            .field("Buttons", "With your Blocks setting on Interactive, or `--interactive` (`-i`) after a request, a stage with more than one crown gets a button for each under the picture. The one shown is green, and one the stage isn't open at is grayed out. Only the person who asked can press them.", false)
            .field("Examples", "`hg!stage earthshaker`\n`hg!s sleeping lion -c 4`\n`hg!s moon -s`", false),
        ("map", _) => Voiced::new(prefix, "hg!map (m)", "`hg!map <name | ID> ...`")
            .field("What you get", prose::PAGE_MAP.said(prose), false)
            .field("Name", "Spaces are fine, and close spellings still match. A name several maps share gets a list to pick from, each with its category and top crown.\n`--search` (`-s`) after it lists the closest matches, 24 to a page", false)
            .field("ID", "A category's prefix and the map: `R 100`, `R-100`, `r100`. A file's prefix works too, so `RN 0` is `N 0`.", false)
            .field("`--crowns 2` (`-c`)", "1 to 4. The stage you pick comes at that crown, and says so if it doesn't have it.", false)
            .field("`--interactive` (`-i`), `--dormant` (`-d`)", "The stage you pick comes with or without its crown buttons, whatever your Blocks setting says.", false)
            .field("Stages", "The list holds 24 stages at a time. ◀ and ▶ under it turn its pages, and the page number between them opens a box to jump to one.", false)
            .field("Invasions", "An invasion is a map of its own, named after the map it invades and marked Invasion. So `hg!map laboratory of relics` offers the map and its invasion, and the Filibuster's is Cats of the Cosmos Ch. 3 with every stage invaded.", false)
            .field("Examples", "`hg!map the legend begins`\n`hg!m N 0 -c 4`\n`hg!m empire of cats ch. 1`", false),
        ("background", _) => Voiced::new(prefix, "hg!background (bg, b)", "`hg!background <ID | stage | map> ...`")
            .field("What you get", prose::PAGE_BACKGROUND.said(prose), false)
            .field("Flags", "`--seed 100` (`-e`): which random run a moving background plays, 100 to start\n`--range 0~299` or `--range 0 299` (`-r`): frames of a moving background, 0~299 to start; one number alone is a single frame\n`--offset 100 50` (`-o`): moves the view right and up from the middle of the screen; negative numbers go left and down\n`--dimensions 800x600` (`-d`): the picture's size, growing outward from that middle; `800 600` and `800×600` work too. 960x640 to start, up to 4096 on a side and 4 megapixels in all\n`--format webp` (`-f`): `avif` (`a`), `webp` (`w`) or `gif` (`g`), this time only, over your default\n`--quality 90` (`-q`): 0 to 100, 80 to start\n`--compress 30` (`-p`): 0 to 100, 0 to start; higher takes longer", false)
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
            .field("Blocks", "Dormant (to start): a cat's, an enemy's or a stage's picture comes alone. Interactive: it comes with buttons under it to change it. `--dormant` (`-d`) and `--interactive` (`-i`) override it for one request.", false)
            .field("Prefix", "What you type in front of a command, which is `hg!` for you right now. Up to 16 characters, no spaces; leave it empty to go back to `{default}`. Mentioning me and `/` always work.", false)
            .field("Prose", "Human (to start) or Tsundere: how I word what I say. What I tell you stays the same.", false)
            .field("Aliases", "Your own names for things, 50 at most. Add takes an alias and what it goes to, like `Dasli` and `Kasli the Bane`. Typing the alias where a name goes then works as if you had typed that instead, and whatever comes after it carries over. An alias beats a real name, and `--search` lists the ones close to what you typed above everything else.\nShow lists them. Remove takes one out, and its Remove All and Set to Default buttons clear them or bring back the ones I come with.", false),
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
    Mode,
    Prose,
    Prefix,
    Alias,
}

impl Setting {
    fn key(self) -> &'static str {
        match self {
            Setting::Language => "lang",
            Setting::Format => "fmt",
            Setting::Form => "form",
            Setting::Search => "search",
            Setting::Mode => "mode",
            Setting::Prose => "prose",
            Setting::Prefix => "prefix",
            Setting::Alias => "alias",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Setting::Language => "Language",
            Setting::Format => "Format",
            Setting::Form => "Form",
            Setting::Search => "Search",
            Setting::Mode => "Blocks",
            Setting::Prose => "Prose",
            Setting::Prefix => "Prefix",
            Setting::Alias => "Aliases",
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
            Setting::Mode => Mode::ALL.into_iter().map(|mode| pick(mode == prefs.mode, format!("set|{owner}|{key}|{}", mode.slug()), mode.label())).collect(),
            Setting::Prose => Prose::ALL.into_iter().map(|prose| pick(prose == prefs.prose, format!("set|{owner}|{key}|{}", prose.slug()), prose.label())).collect(),
            Setting::Prefix => vec![pick(prefs.prefix.is_some(), format!("set|{owner}|{key}"), "Change")],
            Setting::Alias => [("add", "Add"), ("show", "Show"), ("remove", "Remove")].into_iter().map(|(act, label)| pick(false, format!("set|{owner}|{key}|{act}"), label)).collect(),
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
        .field("Blocks", prefs.mode.label(), true)
        .field("Prefix", prefs.prefix(), true)
        .field("Prose", prefs.prose.label(), true)
        .field("Aliases", prefs.aliases.count().to_string(), true);
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

fn alias_list(library: Option<&Library>, prefs: &Prefs) -> Vec<Choice> {
    prefs.aliases.rows().into_iter().map(|row| (clipped(row.name), row.key, clipped(&told(library, prefs, row.target)))).collect()
}

fn alias_pages(act: &str, held: usize) -> usize {
    held.div_ceil(if act == "list" { ALIAS_LINES } else { SEARCH_SHOWN }).max(1)
}

fn alias_shown(owner: u64, list: &[Choice], shown: usize, prose: Prose) -> (serenity::CreateEmbed, Vec<serenity::CreateActionRow>) {
    let pages = alias_pages("list", list.len());
    let shown = shown.min(pages - 1);
    let lines: Vec<String> = list.iter().skip(shown * ALIAS_LINES).take(ALIAS_LINES).map(|(name, _, goes)| format!("`{name}` → {goes}")).collect();
    let text = if lines.is_empty() { prose::ALIAS_NONE.said(prose).to_owned() } else { lines.join("\n") };
    let rows = if pages > 1 { vec![pager(shown, pages, |slot| format!("set|{owner}|alias|list|{slot}"))] } else { Vec::new() };

    (page(prose::ALIAS_TITLE.said(prose), &text), rows)
}

fn alias_removal(owner: u64, aliases: &Aliases, list: &[Choice], shown: usize, note: Option<String>, prose: Prose) -> (String, Vec<serenity::CreateActionRow>) {
    let pages = alias_pages("turn", list.len());
    let shown = shown.min(pages - 1);
    let options: Vec<serenity::CreateSelectMenuOption> = list.iter().skip(shown * SEARCH_SHOWN).take(SEARCH_SHOWN).map(|(name, key, goes)| serenity::CreateSelectMenuOption::new(name, key).description(goes)).collect();
    let menu = (!options.is_empty()).then(|| serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(format!("set|{owner}|alias|pick|{shown}"), serenity::CreateSelectMenuKind::String { options }).placeholder("Pick one to remove")));
    let bulk = |act: &str, label: &str, done: bool| serenity::CreateButton::new(format!("set|{owner}|alias|{act}")).label(label).style(serenity::ButtonStyle::Danger).disabled(done);
    let buttons = turns(shown, pages, |slot| format!("set|{owner}|alias|turn|{slot}")).into_iter().chain([bulk("clear", "Remove All", aliases.cleared()), bulk("reset", "Set to Default", aliases.untouched())]).collect();
    let text = note.unwrap_or_else(|| if list.is_empty() { &prose::ALIAS_NONE } else { &prose::ALIAS_REMOVE_PROMPT }.said(prose).to_owned());

    (text, menu.into_iter().chain([serenity::CreateActionRow::Buttons(buttons)]).collect())
}

fn alias_confirm(owner: u64, act: &str, prose: Prose) -> (String, Vec<serenity::CreateActionRow>) {
    let line = if act == "clear" { &prose::ALIAS_CLEAR_ASK } else { &prose::ALIAS_RESET_ASK };
    let buttons = vec![
        serenity::CreateButton::new(format!("set|{owner}|alias|sure|{act}")).label("Confirm").style(serenity::ButtonStyle::Danger),
        serenity::CreateButton::new(format!("set|{owner}|alias|back")).label("Cancel").style(serenity::ButtonStyle::Secondary),
    ];

    (line.said(prose).to_owned(), vec![serenity::CreateActionRow::Buttons(buttons)])
}

async fn on_alias(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, owner: u64, act: &str, slot: &str) -> Result<(), Error> {
    let prose = data.book.prose(owner);

    if act == "add" {
        let field = |label: &str, id: &str, hint: &str, limit: u16| serenity::CreateActionRow::InputText(serenity::CreateInputText::new(serenity::InputTextStyle::Short, label, id).placeholder(hint).max_length(limit).required(true));
        let modal = serenity::CreateModal::new(SET_ALIAS, "Add an alias").components(vec![field("Alias", "alias", ALIAS_HINT, alias::NAME_LIMIT), field("Goes to", "target", TARGET_HINT, alias::TARGET_LIMIT)]);

        press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
        return Ok(());
    }

    if let Some(shown) = slot.strip_prefix(JUMP).and_then(|shown| shown.parse::<usize>().ok()) {
        press.create_response(ctx, page_box(format!("{ALIAS_JUMP}{act}|{shown}"), shown, alias_pages(act, data.book.aliases(owner).count()))).await?;
        return Ok(());
    }

    if matches!(act, "clear" | "reset") {
        let (text, rows) = alias_confirm(owner, act, prose);

        press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(serenity::CreateInteractionResponseMessage::new().content(text).components(rows))).await?;
        return Ok(());
    }

    let opening = matches!(act, "show" | "remove");
    let first = if opening { serenity::CreateInteractionResponse::Defer(serenity::CreateInteractionResponseMessage::new().ephemeral(true)) } else { serenity::CreateInteractionResponse::Acknowledge };

    press.create_response(ctx, first).await?;

    let mut note = None;
    let prefs = match (act, slot) {
        ("pick", _) => {
            let serenity::ComponentInteractionDataKind::StringSelect { values } = &press.data.kind else { return Ok(()) };
            let Some(key) = values.first() else { return Ok(()) };
            let mut removed = None;
            let prefs = data.book.update(owner, |prefs| removed = prefs.aliases.remove(key));

            note = Some(removed.map_or_else(|| prose::ALIAS_GONE.said(prose).to_owned(), |name| prose::ALIAS_REMOVED.filled(prose, &[("alias", &name)])));
            prefs
        }
        ("sure", "clear") => {
            note = Some(prose::ALIAS_CLEARED.said(prose).to_owned());
            data.book.update(owner, |prefs| prefs.aliases.clear())
        }
        ("sure", "reset") => {
            note = Some(prose::ALIAS_RESET.said(prose).to_owned());
            data.book.update(owner, |prefs| prefs.aliases.reset())
        }
        _ => data.book.get(owner),
    };
    let held = open(data, &prefs.language).await?;
    let list = alias_list(held.as_ref().map(|(_, library)| library.as_ref()), &prefs);
    let shown = slot.parse::<usize>().unwrap_or(0);
    let edit = match act {
        "show" | "list" => {
            let (embed, rows) = alias_shown(owner, &list, shown, prose);

            serenity::EditInteractionResponse::new().embed(embed).components(rows)
        }
        "remove" | "turn" | "pick" | "sure" | "back" => {
            let (text, rows) = alias_removal(owner, &prefs.aliases, &list, shown, note, prose);

            serenity::EditInteractionResponse::new().content(text).components(rows)
        }
        _ => return Ok(()),
    };

    press.edit_response(ctx, edit).await?;
    Ok(())
}

fn field_of(form: &serenity::ModalInteraction, id: &str) -> String {
    form.data
        .components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            serenity::ActionRowComponent::InputText(input) if input.custom_id == id => input.value.clone(),
            _ => None,
        })
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

async fn on_alias_add(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data) -> Result<(), Error> {
    let owner = form.user.id.get();
    let prefs = data.book.get(owner);
    let prose = prefs.prose;
    let (name, target) = (field_of(form, "alias"), field_of(form, "target"));
    let whisper = |text: String| serenity::CreateInteractionResponseFollowup::new().content(text).ephemeral(true);

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    if let Err(line) = alias::readable(&name) {
        form.create_followup(ctx, whisper(line.filled(prose, &[("alias", &name)]))).await?;
        return Ok(());
    }

    if prefs.aliases.full(&name) {
        form.create_followup(ctx, whisper(prose::ALIAS_FULL.filled(prose, &[("limit", &alias::HELD.to_string())]))).await?;
        return Ok(());
    }

    let landing = {
        let Some((_guard, library)) = open(data, &prefs.language).await? else {
            form.create_followup(ctx, whisper(prose::WARMING.said(prose).to_owned())).await?;
            return Ok(());
        };

        lookup::landing(&library, &prefs, &target)
    };
    let added = match landing {
        Landing::Unread(reason) => Err(prose::ALIAS_UNREAD.filled(prose, &[("target", &target), ("alias", &name), ("reason", &reason)])),
        Landing::Nowhere => Err(prose::ALIAS_NOWHERE.filled(prose, &[("target", &target), ("alias", &name)])),
        Landing::One(unit) => Ok(prose::ALIAS_ADDED.filled(prose, &[("alias", &name), ("unit", &unit)])),
        Landing::Several => Ok(prose::ALIAS_ADDED_LIST.filled(prose, &[("alias", &name), ("target", &target)])),
    };
    let told = match added {
        Ok(told) => told,
        Err(reason) => {
            form.create_followup(ctx, whisper(reason)).await?;
            return Ok(());
        }
    };
    let prefs = data.book.update(owner, |prefs| prefs.aliases.add(&name, &target));
    let (embed, rows) = settings_view(owner, &prefs, settings_page("alias"));
    let word = name.split_whitespace().next().map(str::to_lowercase).filter(|word| data.commands.iter().any(|command| command == word));
    let shadowed = word.map(|word| prose::ALIAS_SHADOWED.filled(prose, &[("prefix", prefs.prefix()), ("word", &word), ("alias", &name)]));

    form.edit_response(ctx, serenity::EditInteractionResponse::new().embed(embed).components(rows)).await?;
    form.create_followup(ctx, whisper(std::iter::once(told).chain(shadowed).collect::<Vec<_>>().join("\n"))).await?;
    Ok(())
}

async fn on_alias_jump(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let owner = form.user.id.get();
    let Some((act, shown)) = rest.split_once('|') else { return Ok(()) };
    let prefs = data.book.get(owner);

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    let held = open(data, &prefs.language).await?;
    let list = alias_list(held.as_ref().map(|(_, library)| library.as_ref()), &prefs);
    let asked = typed(form).trim().parse::<usize>().ok().map_or_else(|| shown.parse().unwrap_or(0), |page| page.clamp(1, alias_pages(act, list.len())) - 1);
    let edit = match act {
        "list" => {
            let (embed, rows) = alias_shown(owner, &list, asked, prefs.prose);

            serenity::EditInteractionResponse::new().embed(embed).components(rows)
        }
        "turn" => {
            let (text, rows) = alias_removal(owner, &prefs.aliases, &list, asked, None, prefs.prose);

            serenity::EditInteractionResponse::new().content(text).components(rows)
        }
        _ => return Ok(()),
    };

    form.edit_response(ctx, edit).await?;
    Ok(())
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

    if let Some((owner, page)) = custom.strip_prefix(HELP).and_then(|rest| rest.split_once('|')) {
        if owner.parse::<u64>().ok() != Some(press.user.id.get()) {
            return refuse(ctx, press, data).await;
        }

        let serenity::ComponentInteractionDataKind::StringSelect { values } = &press.data.kind else { return Ok(()) };
        let Some(picked) = values.first().map(String::as_str) else { return Ok(()) };
        let (page, section) = if page.is_empty() { (picked, "") } else { (page, picked) };
        let page = canonical(page).unwrap_or(HOME);
        let section = section_of(page, section);
        let turned = serenity::CreateInteractionResponseMessage::new().embed(help_page(page, section, &prefix_of(data, press.user.id.get()), data.book.prose(press.user.id.get()))).components(help_rows(press.user.id.get(), page, section));

        press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    if let Some((tag, card)) = Card::read(custom) {
        if press.user.id.get() != card.owner {
            return refuse(ctx, press, data).await;
        }

        if tag == CARD_ASK {
            let mut input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, "Magnification (empty goes back to 100%)", "magnification").placeholder(MAGNIFICATION_HINT).max_length(LEVEL_LIMIT).required(false);

            if let Some(held) = card.magnification {
                input = input.value(percents(held));
            }

            let modal = serenity::CreateModal::new(card.turned(CARD_SET, card.page), "Magnification").components(vec![serenity::CreateActionRow::InputText(input)]);

            press.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await?;
            return Ok(());
        }

        press.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

        if let Some(edit) = recarded(data, &card).await? {
            press.edit_response(ctx, edit).await?;
        }

        return Ok(());
    }

    let Some((tag, sheet)) = Sheet::read(custom) else { return Ok(()) };

    if press.user.id.get() != sheet.owner {
        return refuse(ctx, press, data).await;
    }

    match tag {
        "sb" | "sp" => {
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
        "sx" => {
            let input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, "Traits (none or empty turns them off)", "traits").placeholder(VERSUS_HINT).value(grammar::versus_names(sheet.versus)).required(false);
            let modal = serenity::CreateModal::new(sheet.custom("sw", sheet.form), "Versus").components(vec![serenity::CreateActionRow::InputText(input)]);

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

fn aliased<'a>(library: &'a Library, aliases: &Aliases, name: &str) -> Option<&'a EnemyEntry> {
    match aliases.keyed(&fuzzy::normalized(name))? {
        Target::Typed(target) => library.enemy_named(target),
        Target::Known(Kind::Enemy, unit) | Target::Original(Kind::Enemy, unit) => unit.parse().ok().and_then(|id| library.enemy(id)),
        Target::Known(..) | Target::Original(..) => None,
    }
}

fn foes<'a>(library: &'a Library, aliases: &'a Aliases) -> impl Fn(Lookup<'_>) -> Option<Foe> + 'a {
    move |lookup| {
        let found = match lookup {
            Lookup::Numbered(id) => library.enemy(id),
            Lookup::Named(name) => aliased(library, aliases, name).or_else(|| library.enemy_named(name)),
            Lookup::Close(name) => aliased(library, aliases, name).or_else(|| library.find_enemy(name)),
        };

        found.map(|enemy| Foe::of(&enemy.stats))
    }
}

async fn on_versus(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data, mut sheet: Sheet, typed: &str) -> Result<(), Error> {
    let words: Vec<&str> = typed.split_whitespace().collect();
    let prefs = data.book.get(sheet.owner);
    let read = {
        let Some((_guard, library)) = open(data, &prefs.language).await? else {
            let notice = serenity::CreateInteractionResponseMessage::new().content(prose::WARMING.said(prefs.prose)).ephemeral(true);

            form.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
            return Ok(());
        };

        grammar::versus_of(&words, &foes(&library, &prefs.aliases))
    };

    sheet.versus = match read {
        Ok(versus) => versus,
        Err(word) => {
            let notice = serenity::CreateInteractionResponseMessage::new().content(prose::NOT_A_TRAIT.filled(data.book.prose(sheet.owner), &[("name", &word)])).ephemeral(true);

            form.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
            return Ok(());
        }
    };

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    if let Some(edit) = redrawn(data, &sheet).await? {
        form.edit_response(ctx, edit).await?;
    }

    Ok(())
}

async fn on_modal(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data) -> Result<(), Error> {
    if form.data.custom_id == SET_PREFIX {
        return on_prefix(ctx, form, data).await;
    }

    if form.data.custom_id == SET_ALIAS {
        return on_alias_add(ctx, form, data).await;
    }

    if let Some(rest) = form.data.custom_id.strip_prefix(ALIAS_JUMP) {
        return on_alias_jump(ctx, form, data, rest).await;
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

        touched(data, form.message.as_ref().map(|message| message.id), &form.token);
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

        touched(data, form.message.as_ref().map(|message| message.id), &form.token);
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

    if let Some((_, mut card)) = Card::read(&form.data.custom_id).filter(|(tag, card)| *tag == CARD_SET && card.owner == form.user.id.get()) {
        let typed: String = typed(form).split_whitespace().collect();
        let asked = grammar::magnification_of(&typed);

        if !typed.is_empty() && asked.is_none() {
            let notice = serenity::CreateInteractionResponseMessage::new().content(prose::NOT_A_PERCENT.said(data.book.prose(card.owner))).ephemeral(true);

            form.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
            return Ok(());
        }

        form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;
        card.magnification = asked;

        if let Some(edit) = recarded(data, &card).await? {
            form.edit_response(ctx, edit).await?;
        }

        return Ok(());
    }

    let Some((tag, mut sheet)) = Sheet::read(&form.data.custom_id) else { return Ok(()) };

    if form.user.id.get() != sheet.owner {
        return Ok(());
    }

    let typed = typed(form);

    match tag {
        "sv" => return on_level(ctx, form, data, sheet, &typed).await,
        "sw" => return on_versus(ctx, form, data, sheet, &typed).await,
        "sm" => {}
        _ => return Ok(()),
    }

    let prefs = data.book.get(sheet.owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };
    let Some(cat) = library.cat(sheet.id) else { return Ok(()) };
    let words: Vec<&str> = typed.split_whitespace().collect();

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

async fn recarded(data: &Data, card: &Card) -> Result<Option<serenity::EditInteractionResponse>, Error> {
    let prefs = data.book.get(card.owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(None) };
    let rendered = {
        let (library, card, cache) = (Arc::clone(&library), card.clone(), Arc::clone(&data.cache));

        blocking(data, move || card_png(&library, &cache, &card)).await?
    };

    match rendered {
        Ok((bytes, name)) => {
            let attachments = serenity::EditAttachments::new().add(serenity::CreateAttachment::bytes(bytes, name));

            Ok(Some(serenity::EditInteractionResponse::new().attachments(attachments).components(card_rows(card))))
        }
        Err(reason) => {
            error!(enemy = card.id, "A statblock could not be redrawn: {}", reason.told(Prose::Human));
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

    if what == "alias" {
        return on_alias(ctx, press, data, owner, parts.next().unwrap_or_default(), parts.next().unwrap_or_default()).await;
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
        "mode" => prefs.mode = Mode::ALL.into_iter().find(|mode| mode.slug() == value).unwrap_or(prefs.mode),
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
    use codex::combat::{Hits, Mode as Gauge};

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

    // A remark first and the command on the last line is still a command. Only the last line counts,
    // so the request is never mixed with chat and a list of example commands is left alone.
    #[test]
    fn a_command_may_close_a_message() {
        assert_eq!(rest("hey, look!\nhg!c 005", false, None), Some("c 005"));
        assert_eq!(rest("hey, look!\n\n  Hg!c 005", false, None), Some("c 005"));
        assert_eq!(rest("hey, look!\nhg!c 005\n \n", false, None), Some("c 005\n \n"), "blank lines under it do not hide it");
        assert_eq!(rest("hey\nhg!c 005", true, None), Some("c 005"), "in a DM the command still wins over the bare text");
        assert_eq!(rest("<@42> hey\nhg!c 005", false, None), Some("c 005"));
        assert_eq!(rest("try hg!c 005", false, None), None, "mid-line is only talk");
        assert_eq!(rest("use these:\nhg!c 005\nthey are neat", false, None), None);
        assert_eq!(rest("like this:\n```\nhg!c 005\n```", false, None), None);
        assert_eq!(rest("hg!c 005\nlook at him", false, None), Some("c 005\nlook at him"), "a leading command reads on as it always did");
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

    // An alias is swapped in before anything is searched, wherever a name can be typed.
    #[test]
    fn an_alias_is_read_wherever_a_name_goes() {
        let mut aliases = Aliases::default();

        aliases.add("sol", "Stories of Legend");

        assert_eq!(rerouted(&aliases, Scope::Any, "sol"), Some((Scope::Any, "Stories of Legend".to_owned())));
        assert_eq!(rerouted(&aliases, Scope::Only(Kind::Map), "SOL -c 3"), Some((Scope::Only(Kind::Map), "Stories of Legend -c 3".to_owned())));
        assert_eq!(rerouted(&aliases, Scope::Theme, "sol -b"), Some((Scope::Theme, "Stories of Legend --boss".to_owned())));
        assert_eq!(rerouted(&aliases, Scope::Backdrop, "sol -f gif -d 800x600"), Some((Scope::Backdrop, "Stories of Legend -f gif -d 800x600".to_owned())));
        assert_eq!(rerouted(&aliases, Scope::Any, "dasli a walk"), Some((Scope::Only(Kind::Cat), "543-2 a walk".to_owned())), "a built-in one answers as the kind it points at");
        assert_eq!(rerouted(&aliases, Scope::Any, "bun bun"), None, "without a command, a name for the original is settled after the search has picked a kind");
        assert_eq!(rerouted(&aliases, Scope::Only(Kind::Enemy), "bun bun 400%"), Some((Scope::Only(Kind::Enemy), "24 400%".to_owned())));
        assert_eq!(rerouted(&aliases, Scope::Only(Kind::Cat), "bun bun"), None);
        assert_eq!(rerouted(&aliases, Scope::Any, "stories of legend"), None);
        assert_eq!(rerouted(&aliases, Scope::Backdrop, "12"), None);
    }

    // A pick from a search list finds the alias again by its key, in the place the search was made.
    #[test]
    fn a_picked_alias_runs_where_it_was_listed() {
        let mut aliases = Aliases::default();

        aliases.add("sol", "Stories of Legend");

        for scope in [Scope::Any, Scope::Only(Kind::Cat), Scope::Only(Kind::Enemy), Scope::Only(Kind::Stage), Scope::Only(Kind::Map), Scope::Theme, Scope::Backdrop] {
            let tag = format!("{ALIAS}{}", scope.tag());

            assert!(tag.strip_prefix(ALIAS).and_then(Scope::tagged) == Some(scope));
            assert!(!Kind::ALL.iter().any(|kind| kind.tag() == tag) && ![THEME, BACKDROP].contains(&tag.as_str()), "{tag} is already a pick of another sort");
        }

        assert_eq!(chosen(&aliases, Scope::Theme, "sol", BOSS), Some((Scope::Theme, "Stories of Legend --boss".to_owned())));
        assert_eq!(chosen(&aliases, Scope::Theme, "sol", STANDARD), Some((Scope::Theme, "Stories of Legend".to_owned())));
        assert_eq!(chosen(&aliases, Scope::Any, "dasli", "lv50"), Some((Scope::Only(Kind::Cat), "543-2 lv50".to_owned())));
        assert_eq!(chosen(&aliases, Scope::Any, "gone", ""), None);
    }

    // The remove list keeps Discord's limits at every size, and its two bulk buttons gray out once they have nothing left to do.
    #[test]
    fn the_alias_lists_fit_discord() {
        let mut prefs = Prefs::default();

        for slot in 0..alias::HELD {
            prefs.aliases.add(&format!("{slot} {}", "n".repeat(usize::from(alias::NAME_LIMIT) - 3)), &"t".repeat(usize::from(alias::TARGET_LIMIT)));
        }

        let list = alias_list(None, &prefs);
        let pages = alias_pages("turn", list.len());

        assert_eq!(list.len(), prefs.aliases.count());

        for page in 0..pages {
            let rows = serde_json::to_value(alias_removal(u64::MAX, &prefs.aliases, &list, page, None, Prose::Human).1).unwrap();
            let rows = rows.as_array().unwrap();
            let options = rows[0]["components"][0]["options"].as_array().unwrap();

            assert!(rows.len() == 2 && rows[1]["components"].as_array().unwrap().len() == 5 && (1..=25).contains(&options.len()), "page {page}");
            assert!(options.iter().all(|option| ["label", "value", "description"].iter().all(|part| (1..=100).contains(&option[part].as_str().unwrap().chars().count()))));
            assert!(rows.iter().flat_map(|row| row["components"].as_array().unwrap()).all(|part| part["custom_id"].as_str().unwrap().len() <= VALUE_LIMIT));
        }

        for page in 0..alias_pages("list", list.len()) {
            let embed = serde_json::to_value(alias_shown(u64::MAX, &list, page, Prose::Human).0).unwrap();

            assert!(embed["description"].as_str().unwrap().chars().count() <= 4096);
        }

        let bulk = |aliases: &Aliases| -> Vec<bool> {
            let rows = serde_json::to_value(alias_removal(1, aliases, &alias_list(None, &Prefs { aliases: aliases.clone(), ..Prefs::default() }), 0, None, Prose::Human).1).unwrap();
            let buttons = rows.as_array().unwrap().last().unwrap()["components"].as_array().unwrap().clone();

            buttons[3..].iter().map(|button| button["disabled"].as_bool().unwrap_or(false)).collect()
        };
        let mut aliases = Aliases::default();

        assert_eq!(bulk(&aliases), [false, true], "nothing to set back to yet");
        aliases.clear();
        assert_eq!(bulk(&aliases), [true, false]);
        assert_eq!(serde_json::to_value(alias_removal(1, &aliases, &[], 0, None, Prose::Human).1).unwrap().as_array().unwrap().len(), 1, "an empty list has no menu, only its buttons");
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

    // Every help page carries the list of commands with its own page picked, and Cat and Enemy add a list of their sections under it.
    #[test]
    fn help_pages_are_switched_from_lists() {
        let lists = |page: &str, section: &str| -> Vec<(String, Vec<String>, Vec<String>)> {
            serde_json::to_value(help_rows(1, page, section))
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let menu = &row["components"][0];
                    let options = menu["options"].as_array().unwrap();
                    let labels = |picked: bool| options.iter().filter(|option| !picked || option["default"].as_bool() == Some(true)).map(|option| option["label"].as_str().unwrap().to_owned()).collect();

                    (menu["custom_id"].as_str().unwrap().to_owned(), labels(false), labels(true))
                })
                .collect()
        };
        let commands = ["Home", "Cat", "Enemy", "Stage", "Map", "Background", "Audio", "Settings", "Website", "Invite", "Legal", "Help"].map(str::to_owned).to_vec();

        assert_eq!(lists(HOME, ""), [("help|1|".to_owned(), commands.clone(), vec!["Home".to_owned()])]);
        assert_eq!(lists("audio", ""), [("help|1|".to_owned(), commands.clone(), vec!["Audio".to_owned()])]);
        assert_eq!(
            lists("cat", "stats"),
            [("help|1|".to_owned(), commands.clone(), vec!["Cat".to_owned()]), ("help|1|cat".to_owned(), ["General", "Unit", "Stats", "Animation", "Buttons"].map(str::to_owned).to_vec(), vec!["Stats".to_owned()])]
        );
        assert_eq!(lists("enemy", GENERAL)[1], ("help|1|enemy".to_owned(), ["General", "Unit", "Stats", "Animation", "Buttons"].map(str::to_owned).to_vec(), vec!["General".to_owned()]));
        assert_eq!((section_of("cat", ""), section_of("cat", "stats"), section_of("cat", "anim"), section_of("cat", "nonsense"), section_of("enemy", "buttons"), section_of("audio", "unit")), (GENERAL, "stats", "animation", GENERAL, "buttons", ""));

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

    // A reply or a DM has no prefix, so text that only happens to start with a command word is chat, not a request to complain about.
    #[test]
    fn only_a_typed_prefix_makes_a_request() {
        let prefix = |content: &'static str, direct: bool, replied_to: Option<serenity::UserId>| prefix_length(content, prefs::PREFIX, ME, direct, replied_to).map(|length| &content[..length]);

        assert_eq!(prefix("I didnt want that", false, Some(ME)).map(unasked), Some(true));
        assert_eq!(prefix("  invite me please", true, None).map(unasked), Some(true));
        assert_eq!(prefix("hg!i didnt want that", false, Some(ME)).map(unasked), Some(false));
        assert_eq!(prefix("<@42> invite me please", false, None).map(unasked), Some(false));
    }

    // A pick list that is turned or jumped stays alive, one that is answered stops waiting, and nothing else is touched.
    #[test]
    fn a_pick_list_waits_from_its_last_turn() {
        let data_waits: Waits = Arc::new(Mutex::new(HashMap::new()));
        let early = Instant::now();
        let wait = |due: Instant| Wait { due, slip: Slip::Message(serenity::ChannelId::new(1), serenity::MessageId::new(5)), prose: Prose::Human };

        data_waits.lock().unwrap().insert(5, wait(early));
        extend(&mut data_waits.lock().unwrap(), Some(serenity::MessageId::new(5)), "token");
        extend(&mut data_waits.lock().unwrap(), Some(serenity::MessageId::new(6)), "other");
        extend(&mut data_waits.lock().unwrap(), None, "none");

        let held = data_waits.lock().unwrap();

        assert_eq!(held.len(), 1);
        assert!(held[&5].due >= early + PICK_WAIT);
        assert!(matches!(&held[&5].slip, Slip::Reply(token) if token == "token"));
        assert_eq!(prose::PICK_LAPSED.filled(Prose::Human, &[("minutes", &(PICK_WAIT.as_secs() / 60).to_string())]), "Nothing was picked for 5 minutes, so I've dropped that request");
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
        let sheet = Sheet { owner: 1, id: 25, form: 2, level: Some("50+70".to_owned()), talents: Some("10.0.5".to_owned()), page: Page::Talents, versus: Versus { foe: Foe::from_bits(0b1001), aimed: true }, lens: Lens { mode: Gauge::Maximum, hits: Hits::Only(1) } };
        let custom = sheet.custom("sb", 3);
        let (tag, read) = Sheet::read(&custom).unwrap();

        assert_eq!(tag, "sb");
        assert_eq!((read.owner, read.id, read.form, read.level.as_deref(), read.talents.as_deref(), read.page), (1, 25, 3, Some("50+70"), Some("10.0.5"), Page::Talents));
        assert_eq!((read.versus, read.lens), (sheet.versus, sheet.lens));
        assert!(custom.len() <= 100);

        let widest = Sheet { owner: u64::MAX, id: 9999, level: Some("999+999".to_owned()), talents: Some("10.10.10.10.10.10.10.10.10.10".to_owned()), versus: Versus { foe: Foe::from_bits(u16::MAX), aimed: true }, ..sheet.clone() };

        assert!(widest.custom("sb", 3).len() <= 100);

        let automatic = Sheet { level: None, talents: None, ..sheet };
        let (_, read) = Sheet::read(&automatic.custom("sl", 3)).unwrap();

        assert_eq!((read.level, read.talents), (None, None), "no level means the automatic one, and it stays that way through a button");
    }

    // Buttons posted before pages existed carry no page, and they still have to answer.
    #[test]
    fn an_older_sheet_button_opens_the_stats_page() {
        let (_, read) = Sheet::read("sb|1|25|2|50|-").unwrap();

        assert_eq!(read.page, Page::Stats);
        assert!(read.versus.is_empty() && Sheet::read("sb|1|25|2|50|-|s").is_some_and(|(_, read)| read.versus.is_empty()), "and one from before Versus has no traits picked");
        assert_eq!(read.lens, Lens::default(), "and the average of the hits at standing range");
    }

    #[test]
    fn a_card_survives_its_button_id() {
        let card = Card { owner: u64::MAX, id: 763, page: Page::Details, magnification: Some((400, 200)), lens: Lens { mode: Gauge::Minimum, hits: Hits::All } };
        let custom = card.turned(CARD_SET, Page::Stats);
        let (tag, read) = Card::read(&custom).unwrap();

        assert_eq!((tag, read.owner, read.id, read.page, read.magnification, read.lens), (CARD_SET, u64::MAX, 763, Page::Stats, Some((400, 200)), card.lens));
        assert!(custom.len() <= 100);
        assert_eq!(Card::read(&Card { magnification: None, ..card }.turned(CARD_PAGE, Page::Details)).map(|(_, read)| read.magnification), Some(None));
        assert!(Card::read("sb|1|25|2|50|-|s").is_none(), "a cat's button is never read as an enemy's");
        assert!(Sheet::read(&custom).is_none(), "and an enemy's is never read as a cat's");
    }

    // Discord refuses a message whose buttons share an ID, so every button of a panel has its own.
    #[test]
    fn panel_button_ids_never_repeat() {
        let card = Card { owner: 1, id: 2, page: Page::Stats, magnification: None, lens: Lens::default() };
        let ids = [card.turned(CARD_PAGE, Page::Stats), card.turned(CARD_PAGE, Page::Details), card.turned(CARD_ASK, card.page), format!("{LABEL}|0"), format!("{LABEL}|1")];

        for (at, id) in ids.iter().enumerate() {
            assert!(!ids[..at].contains(id), "{id} is used twice");
        }

        let sheet = Sheet { owner: 1, id: 25, form: 2, level: None, talents: None, page: Page::Stats, versus: Versus::default(), lens: Lens::default() };
        let forms = (0..FORMS.len()).map(|form| sheet.turned("sb", form, sheet.page));
        let pages = [Page::Stats, Page::Talents, Page::Details].into_iter().map(|page| sheet.turned("sp", sheet.form, page));
        let ids: Vec<String> = forms.chain(pages).chain([sheet.custom("sl", sheet.form), sheet.custom("st", sheet.form)]).collect();

        for (at, id) in ids.iter().enumerate() {
            assert!(!ids[..at].contains(id), "{id} is used twice");
        }
    }
}
