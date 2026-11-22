use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use poise::serenity_prelude as serenity;
use poise::{BoxFuture, CreateReply, FrameworkError};
use tokio::sync::{OwnedRwLockReadGuard, RwLock, Semaphore};
use tokio::task;
use tracing::{error, info, warn};

use foundry::common::architecture;
use foundry::domains::cat::animation as cat_animation;
use foundry::domains::cat::game::stats::seeded_level;
use foundry::domains::cat::level;
use foundry::domains::enemy::animation as enemy_animation;
use foundry::domains::settings::lang;
use foundry::systems::animation::MotionSet;
use foundry::systems::combat::registry::Magnification;

use super::allowlist;
use super::cache::Cache;
use super::config::Config;
use super::grammar::{self, Kind, Motion, Request, Stats, Unit, Want};
use super::library::{self, Catalog, Library, Remote};
use super::prefs::{self, Book, FORMS, Prefs, Search};
use super::render::{self, Format, Options, Planned, Progress};
use super::talents;

const PREFIX: &str = "hg!";
const REFRESH: Duration = Duration::from_secs(60 * 60);
const TICK: Duration = Duration::from_secs(2);
const BAR: usize = 24;
const TRUE_FORM: usize = 2;
const LICENSES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../foundry/assets/licenses.txt"));
const WARMING: &str = "Still loading the game data, try again in a few minutes";
const INVITE: &str = "If you want to invite me to a server, ask my creator to both whitelist your server ID and to provide to you an invite link!";
const LEGAL: &str = "Not affiliated with PONOS Corp.";
const UNLISTED: &str = "If you want to enable me in a server, ask my creator to whitelist your server ID!";
const NOTICE_TRIES: usize = 5;
const NOT_YOURS: &str = "Only the person who asked can change this one";
const NO_CAT: &str = "Unable to find Cat!";
const NO_ENEMY: &str = "Unable to find Enemy!";
const TALENT_HINT: &str = "all, standard, ultra, none, show, 10 10 5 or Attack Buff 5";
const EMBED_COLOR: u32 = 0x2468A8;
const WEBSITE: &str = "https://hagane.app/";
const OPEN_SETTINGS: &str = "set|open";
const UPLOAD_LIMIT: usize = 20 * 1024 * 1024;
const MEGABYTE: f64 = 1024.0 * 1024.0;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
type Shelf = Arc<RwLock<Option<Arc<Catalog>>>>;
type Guard = OwnedRwLockReadGuard<Option<Arc<Catalog>>>;

pub(crate) struct Data {
    shelf: Shelf,
    cache: Arc<Cache>,
    lanes: Arc<Semaphore>,
    book: Arc<Book>,
}

pub(crate) async fn run(config: Config, cache: Cache) -> Result<(), Error> {
    let shelf: Shelf = Arc::new(RwLock::new(None));
    let lanes = Arc::new(Semaphore::new(thread::available_parallelism().map_or(1, usize::from)));
    let data = Data { shelf: Arc::clone(&shelf), cache: Arc::new(cache), lanes, book: Arc::new(Book::open()) };

    tokio::spawn(keep_fresh(config.source, shelf));

    let options = poise::FrameworkOptions {
        commands: vec![cat(), enemy(), settings(), website(), help(), invite(), legal()],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: None,
            mention_as_prefix: false,
            stripped_dynamic_prefix: Some(invocation),
            case_insensitive_commands: true,
            ..Default::default()
        },
        command_check: Some(|ctx| {
            Box::pin(async move {
                if allowed(ctx.guild_id()) {
                    return Ok(true);
                }

                ctx.say(UNLISTED).await?;
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
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(data)
            })
        })
        .build();
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::ClientBuilder::new(config.token, intents).framework(framework).await?;

    client.start().await?;

    Ok(())
}

fn allowed(guild: Option<serenity::GuildId>) -> bool {
    guild.is_none_or(|guild| allowlist::admits(guild.get()))
}

fn invocation<'a>(ctx: &'a serenity::Context, msg: &'a serenity::Message, _data: &'a Data) -> BoxFuture<'a, Result<Option<&'a str>, Error>> {
    Box::pin(async move {
        let length = prefix_length(&msg.content, ctx.cache.current_user().id, msg.guild_id.is_none(), msg.referenced_message.as_ref().map(|reply| reply.author.id));

        Ok(length.and_then(|length| msg.content.get(..length)))
    })
}

fn prefix_length(content: &str, me: serenity::UserId, direct: bool, replied_to: Option<serenity::UserId>) -> Option<usize> {
    let text = content.trim_start();
    let leading = content.len() - text.len();

    if text.get(..PREFIX.len()).is_some_and(|head| head.eq_ignore_ascii_case(PREFIX)) {
        return Some(leading + PREFIX.len());
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

            if let Err(err) = ctx.say("Something went wrong on my side").await {
                error!("The failure notice could not be sent: {}", err);
            }
        }
        FrameworkError::UnknownCommand { msg, framework, .. } if !allowed(msg.guild_id) => {
            if let Err(err) = msg.channel_id.say(framework.serenity_context, UNLISTED).await {
                warn!(guild = msg.guild_id.map(|guild| guild.get()), "The allowlist notice could not be sent: {}", err);
            }
        }
        FrameworkError::CommandCheckFailed { .. } | FrameworkError::UnknownCommand { .. } => {}
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
    let _lane = Arc::clone(&data.lanes).acquire_owned().await?;

    Ok(task::spawn_blocking(job).await?)
}

fn file(bytes: Vec<u8>, name: String) -> CreateReply {
    CreateReply::default().attachment(serenity::CreateAttachment::bytes(bytes, name))
}

#[derive(Clone)]
struct Sheet {
    owner: u64,
    id: u32,
    form: usize,
    level: String,
    talents: Option<String>,
}

impl Sheet {
    fn custom(&self, tag: &str, form: usize) -> String {
        format!("{tag}|{}|{}|{}|{}|{}", self.owner, self.id, form, self.level, self.talents.as_deref().unwrap_or("-"))
    }

    fn read(custom: &str) -> Option<(&str, Self)> {
        let mut parts = custom.split('|');
        let tag = parts.next()?;
        let owner = parts.next()?.parse().ok()?;
        let id = parts.next()?.parse().ok()?;
        let form = parts.next()?.parse().ok()?;
        let level = parts.next()?.to_owned();
        let talents = parts.next().filter(|levels| *levels != "-").map(str::to_owned);

        Some((tag, Self { owner, id, form, level, talents }))
    }
}

fn sheet_png(library: &Library, cache: &Cache, sheet: &Sheet) -> Result<(Vec<u8>, String), String> {
    let cat = library.cat(sheet.id).ok_or(NO_CAT)?;
    let caps = cat.unitbuy.level_caps();
    let shown = level::capped(&sheet.level, caps);
    let levels = sheet.talents.as_deref().and_then(talents::decoded);
    let parts: [&[u8]; 5] = [b"cat", &sheet.id.to_le_bytes(), &sheet.form.to_le_bytes(), shown.as_bytes(), sheet.talents.as_deref().unwrap_or("-").as_bytes()];
    let bytes = render::remembered(library, cache, &parts, || render::cat_statblock(library, cat, sheet.form, level::total(&shown, caps), &shown, levels.as_ref()))?;

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

    if cat.talent_data.is_some() && sheet.form >= TRUE_FORM {
        let on = sheet.talents.is_some();
        let talents = serenity::CreateButton::new(sheet.custom("st", sheet.form)).label(if on { "Talents: On" } else { "Talents" }).style(if on { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary });

        rows.push(serenity::CreateActionRow::Buttons(vec![talents]));
    }

    rows
}

async fn send_sheet(ctx: Context<'_>, library: Arc<Library>, sheet: Sheet) -> Result<(), Error> {
    let rendered = {
        let (library, sheet) = (Arc::clone(&library), sheet.clone());

        let cache = Arc::clone(&ctx.data().cache);

        blocking(ctx.data(), move || sheet_png(&library, &cache, &sheet)).await?
    };

    match rendered {
        Ok((bytes, name)) => ctx.send(file(bytes, name).components(sheet_rows(&library, &sheet))).await?,
        Err(reason) => ctx.say(reason).await?,
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

async fn animate(ctx: Context<'_>, library: Arc<Library>, set: MotionSet, options: Options, title: String) -> Result<(), Error> {
    let (clip, format, options_looping) = (options.clip, options.format, options.looping.is_some());
    let cache = Arc::clone(&ctx.data().cache);
    let planned = {
        let (library, cache) = (Arc::clone(&library), Arc::clone(&cache));

        task::spawn_blocking(move || render::plan(&library, &cache, &set, &options)).await?
    };
    let job = match planned {
        Ok(None) => {
            ctx.say(format!("{title} has no {} animation", clip.motion())).await?;
            return Ok(());
        }
        Ok(Some(Planned::Ready(bytes, _))) if bytes.len() > UPLOAD_LIMIT => {
            ctx.say(oversized(bytes.len())).await?;
            return Ok(());
        }
        Ok(Some(Planned::Ready(bytes, name))) => {
            ctx.send(file(bytes, name)).await?;
            return Ok(());
        }
        Ok(Some(Planned::Encode(job))) => job,
        Err(reason) if reason.starts_with("That's ") => {
            ctx.say(reason).await?;
            return Ok(());
        }
        Err(reason) => {
            error!(title, clip = clip.slug(), "An animation could not be prepared: {}", reason);
            ctx.say(format!("I couldn't make that one: {reason}")).await?;
            return Ok(());
        }
    };
    let label = format!("{title} • {} • {}", clip.motion(), format.label());
    let searching = options_looping;
    let started = Instant::now();
    let progress = Arc::new(Progress::default());
    let handle = ctx.send(CreateReply::default().content(progress_bar(&label, searching, &progress, Duration::ZERO))).await?;
    let _lane = Arc::clone(&ctx.data().lanes).acquire_owned().await?;
    let mut work = {
        let progress = Arc::clone(&progress);

        task::spawn_blocking(move || job.run(&cache, &progress))
    };
    let finished = loop {
        tokio::select! {
            done = &mut work => break done?,
            () = tokio::time::sleep(TICK) => {
                if let Err(err) = handle.edit(ctx, CreateReply::default().content(progress_bar(&label, searching, &progress, started.elapsed()))).await {
                    warn!("The progress bar could not be updated: {}", err);
                }
            }
        }
    };

    match finished {
        Ok((bytes, _)) if bytes.len() > UPLOAD_LIMIT => handle.edit(ctx, CreateReply::default().content(oversized(bytes.len()))).await?,
        Ok((bytes, name)) => handle.edit(ctx, file(bytes, name).content(String::new())).await?,
        Err(reason) if reason == render::NO_LOOP || reason == render::TOO_LONG || reason.starts_with("That's ") => {
            handle.edit(ctx, CreateReply::default().content(reason)).await?;
        }
        Err(reason) => {
            error!(title, clip = clip.slug(), "An animation could not be exported: {}", reason);
            handle.edit(ctx, CreateReply::default().content(format!("I couldn't make that one: {reason}"))).await?;
        }
    }

    Ok(())
}

fn oversized(length: usize) -> String {
    format!("That one comes out at {:.1} MB, over Discord's 20 MB upload limit. Try `--format avif`, which is far smaller", length as f64 / MEGABYTE)
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

fn settled(request: Request, named: impl Fn(&str) -> bool) -> Request {
    match request.whole {
        Some(whole) if named(&whole) => Request { unit: Unit::Name(whole), want: Want::Stats(Stats::default()), whole: None, form: request.form },
        _ => request,
    }
}

async fn understood(ctx: Context<'_>, request: Option<String>, kind: Kind) -> Result<Option<Request>, Error> {
    match grammar::parse(request.as_deref().unwrap_or_default(), kind) {
        Ok(request) => Ok(Some(request)),
        Err(reason) => {
            ctx.say(format!("{reason}. See `hg!help`")).await?;
            Ok(None)
        }
    }
}

async fn working(ctx: Context<'_>) -> Result<(), Error> {
    match ctx {
        poise::Context::Application(_) => ctx.defer().await?,
        poise::Context::Prefix(_) => {
            if let Err(err) = ctx.http().broadcast_typing(ctx.channel_id()).await {
                warn!("The typing indicator could not be shown: {}", err);
            }
        }
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("c"))]
async fn cat(ctx: Context<'_>, #[description = "ID-form or name, then stats or anim"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Cat).await? else { return Ok(()) };
    let owner = ctx.author().id.get();
    let prefs = ctx.data().book.get(owner);
    working(ctx).await?;
    let Some((_guard, library)) = open(ctx.data(), &prefs.language).await? else {
        ctx.say(WARMING).await?;
        return Ok(());
    };
    let request = settled(request, |name| library.names_cat(name));
    let found = match &request.unit {
        Unit::Id(id, form) => library.cat(*id).map(|cat| (cat, form.unwrap_or_else(|| prefs.form_from(&cat.forms)))),
        Unit::Name(name) => library.find_cat(name, &prefs),
    };
    let found = found.map(|(cat, form)| (cat, request.form.unwrap_or(form)));
    let Some((cat, form)) = found else {
        ctx.say(NO_CAT).await?;
        return Ok(());
    };

    if !cat.forms.get(form).copied().unwrap_or(false) {
        ctx.say(format!("{} has no {} form", cat.display_name(0), FORMS.get(form).copied().unwrap_or("such"))).await?;
        return Ok(());
    }

    match request.want {
        Want::Stats(stats) if stats.talents.as_deref().is_some_and(talents::shows) => {
            let form = if form < TRUE_FORM && cat.forms[TRUE_FORM] { TRUE_FORM } else { form };

            ctx.say(talents::overview(&library, cat, form)).await?;
            Ok(())
        }
        Want::Stats(stats) => {
            let level = stats.level.unwrap_or_else(|| seeded_level(cat, &library.settings).1);
            let talents = match stats.talents {
                Some(words) => match talents::levels(&library, cat, &words) {
                    Ok(levels) => talents::encoded(&levels, cat.talent_data.as_ref().map_or(0, |data| data.groups.len())),
                    Err(reason) => {
                        ctx.say(reason).await?;
                        return Ok(());
                    }
                },
                None => None,
            };
            let form = if talents.is_some() && form < TRUE_FORM && cat.forms[TRUE_FORM] { TRUE_FORM } else { form };
            let sheet = Sheet { owner, id: cat.id, form, level, talents };

            send_sheet(ctx, Arc::clone(&library), sheet).await
        }
        Want::Motion(motion) => {
            let set = cat_animation::motions(cat, form, &library.vault.vfs);
            let title = cat.display_name(form);

            animate(ctx, Arc::clone(&library), set, options_of(&motion, &prefs), title).await
        }
    }
}

#[poise::command(slash_command, prefix_command, aliases("e"))]
async fn enemy(ctx: Context<'_>, #[description = "ID or name, then stats or anim"] #[rest] request: Option<String>) -> Result<(), Error> {
    let Some(request) = understood(ctx, request, Kind::Enemy).await? else { return Ok(()) };
    let prefs = ctx.data().book.get(ctx.author().id.get());
    working(ctx).await?;
    let Some((_guard, library)) = open(ctx.data(), &prefs.language).await? else {
        ctx.say(WARMING).await?;
        return Ok(());
    };
    let request = settled(request, |name| library.names_enemy(name));
    let found = match &request.unit {
        Unit::Id(id, _) => library.enemy(*id),
        Unit::Name(name) => library.find_enemy(name),
    };
    let Some(enemy) = found else {
        ctx.say(NO_ENEMY).await?;
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
                let (library, cache) = (Arc::clone(&library), Arc::clone(&ctx.data().cache));

                blocking(ctx.data(), move || {
                    let enemy = library.enemy(id).ok_or(NO_ENEMY)?;
                    let parts: [&[u8]; 4] = [b"enemy", &id.to_le_bytes(), &hitpoints.to_le_bytes(), &attack.to_le_bytes()];

                    render::remembered(&library, &cache, &parts, || render::enemy_statblock(&library, enemy, Magnification { hitpoints, attack })).map(|bytes| (bytes, format!("{}.png", enemy.id_str())))
                })
                .await?
            };

            match rendered {
                Ok((bytes, name)) => ctx.send(file(bytes, name)).await?,
                Err(reason) => ctx.say(reason).await?,
            };

            Ok(())
        }
        Want::Motion(motion) => {
            let set = enemy_animation::motions(enemy, &library.vault.vfs);
            let title = enemy.display_name();

            animate(ctx, Arc::clone(&library), set, options_of(&motion, &prefs), title).await
        }
    }
}

#[poise::command(slash_command, prefix_command, aliases("h"))]
async fn help(ctx: Context<'_>, #[description = "A command to explain, like cat or enemy"] #[rest] command: Option<String>) -> Result<(), Error> {
    let asked = command.as_deref().map(str::trim).filter(|asked| !asked.is_empty());
    let page = match asked {
        None => Some(HOME),
        Some(asked) => canonical(&asked.trim_start_matches(PREFIX).to_lowercase()),
    };
    let owner = ctx.author().id.get();

    match page {
        Some(page) => ctx.send(CreateReply::default().embed(help_page(page)).components(help_rows(owner, page))).await?,
        None => ctx.say(format!("I don't have a command called `{}`. See `hg!help`", asked.unwrap_or_default())).await?,
    };

    Ok(())
}

const HOME: &str = "home";
const PAGES: [(&str, &str); 8] = [(HOME, "Home"), ("cat", "Cat"), ("enemy", "Enemy"), ("settings", "Settings"), ("website", "Website"), ("invite", "Invite"), ("legal", "Legal"), ("help", "Help")];
const PAGES_PER_ROW: usize = 4;

fn canonical(asked: &str) -> Option<&'static str> {
    match asked {
        "home" | "general" => Some(HOME),
        "cat" | "c" => Some("cat"),
        "enemy" | "e" => Some("enemy"),
        "settings" | "s" | "set" => Some("settings"),
        "website" | "web" | "w" => Some("website"),
        "invite" | "i" | "inv" => Some("invite"),
        "legal" | "l" | "license" => Some("legal"),
        "help" | "h" => Some("help"),
        _ => None,
    }
}

fn help_rows(owner: u64, current: &str) -> Vec<serenity::CreateActionRow> {
    PAGES
        .chunks(PAGES_PER_ROW)
        .map(|row| {
            let buttons = row
                .iter()
                .map(|(page, label)| {
                    let here = *page == current;

                    serenity::CreateButton::new(format!("help|{owner}|{page}")).label(*label).style(if here { serenity::ButtonStyle::Primary } else { serenity::ButtonStyle::Secondary }).disabled(here)
                })
                .collect();

            serenity::CreateActionRow::Buttons(buttons)
        })
        .collect()
}

fn page(title: &str, description: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new().title(title).description(description).color(EMBED_COLOR)
}

const CLIPS: &str = "`attack` (`atk`, `a`), `idle` (`i`), `walk` (`w`), `knockback` (`kb`, `k`), `showcase` (`sh`, `sc`), `burrow` (`bur`, `b`), `surface` (`su`, `sf`, `rise`, `r`), `dig` (`d`), `spirit` (`sp`, `c`, `conjure`). Attack if none is given.";
const MOTION_FLAGS: &str = "`--range 0~30` or `--range 0 30` (`-r`): frames to export, wrapping around the loop; not for showcases
`--loop` (`-l`), walk and idle only: finds a perfect loop; `min 15`, `max 200` and `diff 30` tune it, and with `--range` it only searches that window
`--format webp` (`-f`): `avif`, `webp` or `gif`, this time only, over your default
`--background` (`-b`): a solid gray background instead of a transparent one
`--quality 90` (`-q`): 0 to 100, 80 to start
`--compress 30` (`-p`): 0 to 100, 0 to start; higher takes longer
`--cull 40`: bounds cull %, for long beams and faint effects; spirits are never culled
`--combat` (`-c`), `--rig` (`-g`), `--world` (`-w`), `--origin` (`-o`): debug overlays";

fn help_page(page_name: &str) -> serenity::CreateEmbed {
    match page_name {
        "cat" => page("hg!cat (c)", "`hg!cat <unit> [stats | animation] ...`")
            .field("Unit", "`25-3`, `25_t` or `25_s`: an ID and a form (1-4, n e t u, or f c s u)\n`25` or `025`: your default form, or the next one down if it has none\nA name: spaces are fine, close spellings still match\n`form` (`f`) after either picks a form: 1-4, n e t u, f c s u, or Normal, Evolved, True, Ultra", false)
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a level or `--talents` means stats. `s` or `a` still decides outright, and stats is the default.", false)
            .field("Stats (`s`, `stat`)", "A level: `lv50+70`, `lvl50`, `l50`, `level50` or just `50`. Without one, a typical level for its rarity.\n`--talents` (`-t`): `all` (`a`), `standard` (`s`), `ultra` (`u`), `none` (`n`), `show` (`sh`) to list them with their max levels, levels in order like `10 10 5`, or names like `Attack Buff 5`. Talents lift a Normal or Evolved form to True.", false)
            .field("Animation (`a`, `anim`)", CLIPS, false)
            .field("Animation flags", MOTION_FLAGS, false)
            .field("Buttons", "Statblocks get a button per form, and Talents on True and Ultra forms. Only the person who asked can press them.", false)
            .field("Examples", "`hg!c 025_u lv50+70 -t all`\n`hg!c bahamut cat form normal`\n`hg!c jamiera cat a showcase -f webp`\n`hg!c 025 a walk -l -r 10~120`", false),
        "enemy" => page("hg!enemy (e)", "`hg!enemy <unit> [stats | animation] ...`")
            .field("Unit", "`763` or `0763`: an ID\nA name: spaces are fine, close spellings still match", false)
            .field("Stats or animation", "Worked out from what you type: an animation name or flag means animation, a percentage means stats. `s` or `a` still decides outright, and stats is the default.", false)
            .field("Stats (`s`, `stat`)", "A magnification: `400%`, or `400%/200%` for hitpoints and attack. 100% without one.", false)
            .field("Animation (`a`, `anim`)", CLIPS, false)
            .field("Animation flags", MOTION_FLAGS, false)
            .field("Examples", "`hg!e akaza 400%`, `hg!e akaza s 400`\n`hg!e 763 a attack -c -w`", false),
        "settings" => page("hg!settings (s, set)", "Your own defaults, changed with buttons. `/settings` opens them just for you; `hg!settings` gives a button that opens yours privately.")
            .field("Language", "Every language the game data has. It also changes which names searches match. English to start.", false)
            .field("Format", "AVIF, WebP or GIF, for animations. AVIF to start, and the smallest by far. `--format` overrides it for one request.", false)
            .field("Default form", "The form a bare ID gets, or a name that matches several forms of one cat. True to start; a cat without it gets the next form down.", false)
            .field("Search", "Exact (to start): a form's exact name gets that form, anything else is a close search. Preferred: a found cat always gets your default form.", false),
        "website" => page("hg!website (web, w)", "A link to the Hagane website."),
        "invite" => page("hg!invite (inv, i)", "How to get me into another server."),
        "legal" => page("hg!legal (license, l)", "The licenses for everything I'm built from, and who I'm not affiliated with."),
        "help" => page("hg!help (h)", "`hg!help` lists every command. `hg!help <command>` opens one, aliases included, like `hg!help c`. The buttons below switch between them."),
        _ => page("Hagane", "Use `hg!`, mention me, reply to me, DM me, or use `/`.\nThe buttons below open each command, or use `hg!help <command>`.")
            .field("`hg!cat` (`c`)", "A cat's statblock or animation, by ID or name", false)
            .field("`hg!enemy` (`e`)", "An enemy's statblock or animation, by ID or name", false)
            .field("`hg!settings` (`s`, `set`)", "Your language, image format and default form", false)
            .field("`hg!website` (`web`, `w`)", "A link to the Hagane website", false)
            .field("`hg!invite` (`inv`, `i`)", "How to get me into a server", false)
            .field("`hg!legal` (`license`, `l`)", "Licenses, and who I'm not affiliated with", false)
            .field("`hg!help` (`h`)", "This list, or one command's page", false),
    }
}

#[poise::command(slash_command, prefix_command, aliases("web", "w"))]
async fn website(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(WEBSITE).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("inv", "i"))]
async fn invite(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(INVITE).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("license", "l"))]
async fn legal(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(file(LICENSES.to_vec(), "licenses.txt".to_owned()).content(LEGAL)).await?;
    Ok(())
}

const FLAGS: [(&str, &str); 9] = [("en", "🇺🇸"), ("ja", "🇯🇵"), ("tw", "🇹🇼"), ("ko", "🇰🇷"), ("es", "🇪🇸"), ("de", "🇩🇪"), ("fr", "🇫🇷"), ("it", "🇮🇹"), ("th", "🇹🇭")];

fn flag(code: &str) -> &'static str {
    FLAGS.iter().find(|(held, _)| *held == code).map_or("🌐", |(_, flag)| flag)
}

fn settings_view(owner: u64, prefs: &Prefs) -> (serenity::CreateEmbed, Vec<serenity::CreateActionRow>) {
    let form = FORMS.get(prefs.form).copied().unwrap_or("True");
    let embed = page("Your settings", "Changes save as soon as you press them.")
        .field("Language", format!("{} {}", flag(&prefs.language), prefs::language_name(&prefs.language)), true)
        .field("Format", prefs.format.label(), true)
        .field("Default form", form, true)
        .field("Search", prefs.search.label(), true);
    let languages: Vec<serenity::CreateSelectMenuOption> = lang::choices()
        .map(|(code, name)| serenity::CreateSelectMenuOption::new(name, code).emoji(serenity::ReactionType::Unicode(flag(code).to_owned())).default_selection(code == prefs.language))
        .collect();
    let caption = |custom: String, label: &str| serenity::CreateButton::new(custom).label(label).style(serenity::ButtonStyle::Secondary).disabled(true);
    let pick = |current: bool, custom: String, label: &str| serenity::CreateButton::new(custom).label(label).style(if current { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary });
    let formats = std::iter::once(caption(format!("set|{owner}|caption|fmt"), "Format"))
        .chain([Format::Avif, Format::WebP, Format::Gif].into_iter().map(|format| pick(format == prefs.format, format!("set|{owner}|fmt|{}", format.extension()), format.label())))
        .collect();
    let forms = std::iter::once(caption(format!("set|{owner}|caption|form"), "Form"))
        .chain(FORMS.iter().enumerate().map(|(form, label)| pick(form == prefs.form, format!("set|{owner}|form|{form}"), label)))
        .collect();
    let searches = std::iter::once(caption(format!("set|{owner}|caption|search"), "Search"))
        .chain([Search::Exact, Search::Preferred].into_iter().map(|search| pick(search == prefs.search, format!("set|{owner}|search|{}", search.slug()), search.label())))
        .collect();
    let rows = vec![
        serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(format!("set|{owner}|lang"), serenity::CreateSelectMenuKind::String { options: languages }).placeholder("Language")),
        serenity::CreateActionRow::Buttons(formats),
        serenity::CreateActionRow::Buttons(forms),
        serenity::CreateActionRow::Buttons(searches),
    ];

    (embed, rows)
}

#[poise::command(slash_command, prefix_command, aliases("set", "s"))]
async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    if let poise::Context::Prefix(_) = ctx {
        let open = serenity::CreateButton::new(OPEN_SETTINGS).label("Open Settings").style(serenity::ButtonStyle::Primary);

        ctx.send(CreateReply::default().content("Settings are private, so they open just for whoever presses this:").components(vec![serenity::CreateActionRow::Buttons(vec![open])])).await?;
        return Ok(());
    }

    let owner = ctx.author().id.get();
    let (embed, rows) = settings_view(owner, &ctx.data().book.get(owner));

    ctx.send(CreateReply::default().embed(embed).components(rows).ephemeral(true)).await?;
    Ok(())
}

async fn on_event(ctx: &serenity::Context, event: &serenity::FullEvent, data: &Data) -> Result<(), Error> {
    match event {
        serenity::FullEvent::GuildCreate { guild, .. } if !allowed(Some(guild.id)) => {
            warn!(guild = guild.id.get(), name = guild.name, "Leaving a server that is not on the allowlist");
            tell_unlisted(ctx, guild).await;

            if let Err(err) = guild.id.leave(ctx).await {
                error!(guild = guild.id.get(), "Could not leave a server that is not on the allowlist: {}", err);
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

        if channel.say(ctx, UNLISTED).await.is_ok() {
            return;
        }
    }

    warn!(guild = guild.id.get(), "No channel would take the allowlist notice");
}

async fn refuse(ctx: &serenity::Context, press: &serenity::ComponentInteraction) -> Result<(), Error> {
    let notice = serenity::CreateInteractionResponseMessage::new().content(NOT_YOURS).ephemeral(true);

    press.create_response(ctx, serenity::CreateInteractionResponse::Message(notice)).await?;
    Ok(())
}

async fn on_press(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data) -> Result<(), Error> {
    let custom = press.data.custom_id.as_str();

    if custom == OPEN_SETTINGS {
        let owner = press.user.id.get();
        let (embed, rows) = settings_view(owner, &data.book.get(owner));
        let private = serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows).ephemeral(true);

        press.create_response(ctx, serenity::CreateInteractionResponse::Message(private)).await?;
        return Ok(());
    }

    if let Some(rest) = custom.strip_prefix("set|") {
        return on_setting(ctx, press, data, rest).await;
    }

    if let Some((owner, page)) = custom.strip_prefix("help|").and_then(|rest| rest.split_once('|')) {
        if owner.parse::<u64>().ok() != Some(press.user.id.get()) {
            return refuse(ctx, press).await;
        }

        let page = canonical(page).unwrap_or(HOME);
        let turned = serenity::CreateInteractionResponseMessage::new().embed(help_page(page)).components(help_rows(press.user.id.get(), page));

        press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(turned)).await?;
        return Ok(());
    }

    let Some((tag, sheet)) = Sheet::read(custom) else { return Ok(()) };

    if press.user.id.get() != sheet.owner {
        return refuse(ctx, press).await;
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
        _ => Ok(()),
    }
}

async fn on_modal(ctx: &serenity::Context, form: &serenity::ModalInteraction, data: &Data) -> Result<(), Error> {
    let Some(("sm", mut sheet)) = Sheet::read(&form.data.custom_id) else { return Ok(()) };

    if form.user.id.get() != sheet.owner {
        return Ok(());
    }

    let typed = form
        .data
        .components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            serenity::ActionRowComponent::InputText(input) => input.value.clone(),
            _ => None,
        })
        .unwrap_or_default();

    let prefs = data.book.get(sheet.owner);
    let Some((_guard, library)) = open(data, &prefs.language).await? else { return Ok(()) };
    let Some(cat) = library.cat(sheet.id) else { return Ok(()) };
    let words: Vec<&str> = typed.split_whitespace().collect();

    if talents::shows(&grammar::talents_of(&words)) {
        let private = serenity::CreateInteractionResponseMessage::new().content(talents::overview(&library, cat, sheet.form)).ephemeral(true);

        form.create_response(ctx, serenity::CreateInteractionResponse::Message(private)).await?;
        return Ok(());
    }

    form.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge).await?;

    sheet.talents = if words.is_empty() {
        None
    } else {
        match talents::levels(&library, cat, &grammar::talents_of(&words)) {
            Ok(levels) => talents::encoded(&levels, cat.talent_data.as_ref().map_or(0, |held| held.groups.len())),
            Err(reason) => {
                let notice = serenity::CreateInteractionResponseFollowup::new().content(format!("{reason}\n{}", talents::listed(&library, cat, None))).ephemeral(true);

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
            error!(cat = sheet.id, "A statblock could not be redrawn: {}", reason);
            Ok(None)
        }
    }
}

async fn on_setting(ctx: &serenity::Context, press: &serenity::ComponentInteraction, data: &Data, rest: &str) -> Result<(), Error> {
    let mut parts = rest.split('|');
    let Some(owner) = parts.next().and_then(|owner| owner.parse::<u64>().ok()) else { return Ok(()) };

    if press.user.id.get() != owner {
        return refuse(ctx, press).await;
    }

    let what = parts.next().unwrap_or_default();
    let value = match &press.data.kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => values.first().cloned().unwrap_or_default(),
        _ => parts.next().unwrap_or_default().to_owned(),
    };
    let prefs = data.book.update(owner, |prefs| match what {
        "lang" if lang::choices().any(|(code, _)| code == value) => prefs.language = value,
        "fmt" => prefs.format = [Format::Avif, Format::WebP, Format::Gif].into_iter().find(|format| format.extension() == value).unwrap_or(prefs.format),
        "form" => prefs.form = value.parse::<usize>().ok().filter(|form| *form < FORMS.len()).unwrap_or(prefs.form),
        "search" => prefs.search = [Search::Exact, Search::Preferred].into_iter().find(|search| search.slug() == value).unwrap_or(prefs.search),
        _ => {}
    });
    let (embed, rows) = settings_view(owner, &prefs);
    let updated = serenity::CreateInteractionResponseMessage::new().embed(embed).components(rows);

    press.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(updated)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: serenity::UserId = serenity::UserId::new(42);
    const SOMEONE: serenity::UserId = serenity::UserId::new(7);

    // Poise wants the prefix itself back and strips it on its own, so check what it is left with.
    fn rest(content: &str, direct: bool, replied_to: Option<serenity::UserId>) -> Option<&str> {
        prefix_length(content, ME, direct, replied_to).map(|length| content[length..].trim_start())
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
        assert_eq!(prefix_length("hg!cat 25", ME, false, None).map(|length| &"hg!cat 25"[..length]), Some("hg!"));
    }

    // Ordinary chat must never be read as a command.
    #[test]
    fn plain_chat_is_left_alone() {
        assert_eq!(rest("cat 25", false, None), None);
        assert_eq!(rest("cat 25", false, Some(SOMEONE)), None);
        assert_eq!(rest("!hg cat 25", false, None), None);
        assert_eq!(rest("<@7> cat 25", false, None), None);
    }

    #[test]
    fn a_sheet_survives_its_button_id() {
        let sheet = Sheet { owner: 1, id: 25, form: 2, level: "50+70".to_owned(), talents: Some("10.0.5".to_owned()) };
        let custom = sheet.custom("sb", 3);
        let (tag, read) = Sheet::read(&custom).unwrap();

        assert_eq!(tag, "sb");
        assert_eq!((read.owner, read.id, read.form, read.level.as_str(), read.talents.as_deref()), (1, 25, 3, "50+70", Some("10.0.5")));
        assert!(custom.len() <= 100);
    }
}
