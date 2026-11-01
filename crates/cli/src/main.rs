//! Interactive host and browser-client commands. Capabilities stay on live terminals.
use qrcode::{QrCode, render::unicode};
use rand::RngCore;
use ratatui::crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io::{self, IsTerminal, Write},
    net::{SocketAddr, UdpSocket},
    process::ExitCode,
    time::Duration,
};
use tokio::{net::TcpListener, signal};
use tokio_util::sync::CancellationToken;
use tunnel_core::{
    Failure,
    peer_route::{self, Helper, HelperConfig, Offer},
};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Failure>;

#[cfg(test)]
#[path = "../tests/internal/bind_tests.rs"]
mod bind_tests;

const USAGE: &str = "Usage: thepeer share HOST:PORT [--bind IP:PORT] [--observer IP:PORT] [--relay IP:PORT]\n       thepeer connect\nShare one localhost HTTP application with a terminal or the matching Android app.\nConnect prompts privately for the one-shot connection string, then prints a local browser URL.\nAutomatically select a local source address toward the participant; --bind overrides this on the host.\nPlain HTTP target: hostname:PORT or [IPv6]:PORT. Resolve once; every address must be loopback.\nParticipant options perform network bootstrap; no relay is operated by this CLI.\nInteractive terminal required; no automatic tunnel reconnection or coordinated punching.";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(stage)) => {
            eprintln!("Helper failed: {stage}; no fallback listener started.");
            ExitCode::FAILURE
        }
    }
}

struct Arguments {
    bind: Option<SocketAddr>,
    target: tunnel_core::target::Target,
    observer: Option<SocketAddr>,
    relay: Option<SocketAddr>,
}

fn arguments(args: &[String]) -> Result<Arguments> {
    let (mut bind, mut observer, mut relay) = (None, None, None);
    let Some(first) = args.first() else {
        return Err(Failure("helper-arguments"));
    };
    let target = first.parse().map_err(|_| Failure("helper-arguments"))?;
    let flags = &args[1..];
    if flags.len() > 6 || flags.len() % 2 != 0 {
        return Err(Failure("helper-arguments"));
    }
    for pair in flags.chunks_exact(2) {
        match pair[0].as_str() {
            "--bind" if bind.is_none() => bind = Some(address(&pair[1], true)?),
            "--observer" if observer.is_none() => observer = Some(address(&pair[1], false)?),
            "--relay" if relay.is_none() => relay = Some(address(&pair[1], false)?),
            _ => return Err(Failure("helper-arguments")),
        }
    }
    Ok(Arguments {
        bind,
        target,
        observer,
        relay,
    })
}

impl Arguments {
    fn resolve(self) -> Result<(HelperConfig, Option<SocketAddr>)> {
        if self
            .observer
            .zip(self.relay)
            .is_some_and(|(a, b)| a.is_ipv4() != b.is_ipv4())
        {
            return Err(Failure("route-family"));
        }
        let bind = match self.bind {
            Some(bind) => bind,
            None => discover_bind(self.observer.or(self.relay).unwrap_or_else(|| {
                // Documentation-only address: UDP connect consults the routing
                // table; no send, DNS, public probe or relay request is performed.
                SocketAddr::from(([192, 0, 2, 1], 9))
            }))?,
        };
        Ok((
            HelperConfig {
                bind,
                target: self.target,
                observer: self.observer,
            },
            self.relay,
        ))
    }
}

fn discover_bind(destination: SocketAddr) -> Result<SocketAddr> {
    let wildcard = if destination.is_ipv4() {
        SocketAddr::from(([0, 0, 0, 0], 0))
    } else {
        SocketAddr::from(([0u16; 8], 0))
    };
    let socket = UdpSocket::bind(wildcard).map_err(|_| Failure("helper-bind-discovery"))?;
    socket
        .connect(destination)
        .map_err(|_| Failure("helper-bind-discovery"))?;
    let mut selected = socket
        .local_addr()
        .map_err(|_| Failure("helper-bind-discovery"))?;
    // The lookup socket is not the transport and its port is never advertised.
    selected.set_port(0);
    address(&selected.to_string(), true).map_err(|_| Failure("helper-bind-discovery"))
}

fn address(text: &str, ephemeral: bool) -> Result<SocketAddr> {
    let address: SocketAddr = text.parse().map_err(|_| Failure("helper-address"))?;
    let ip = address.ip();
    if (!ephemeral && address.port() == 0)
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip == std::net::Ipv4Addr::BROADCAST
        || matches!(address, SocketAddr::V6(v6) if v6.scope_id() != 0 || v6.flowinfo() != 0
            || v6.ip().is_unicast_link_local() || v6.ip().to_ipv4_mapped().is_some())
    {
        return Err(Failure("helper-address"));
    }
    Ok(address)
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(Failure("helper-terminal"));
    }
    match args.as_slice() {
        [command] if command == "connect" => connect().await,
        [command, rest @ ..] if command == "share" => share(rest).await,
        _ => Err(Failure("helper-arguments")),
    }
}

// Hidden, bounded interactive input: the live grant never appears in argv or history.
fn prompt_offer() -> Result<Offer> {
    struct PromptRestore;
    impl Drop for PromptRestore {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), DisableBracketedPaste);
            let _ = terminal::disable_raw_mode();
            let _ = writeln!(io::stdout());
        }
    }
    terminal::enable_raw_mode().map_err(|_| Failure("connect-terminal"))?;
    let _restore = PromptRestore;
    execute!(io::stdout(), EnableBracketedPaste).map_err(|_| Failure("connect-terminal"))?;
    print!("Paste connection string (hidden), then press Enter: ");
    io::stdout()
        .flush()
        .map_err(|_| Failure("connect-terminal"))?;
    let mut text = Zeroizing::new(String::new());
    loop {
        match event::read().map_err(|_| Failure("connect-terminal"))? {
            Event::Paste(paste) => {
                let paste = Zeroizing::new(paste);
                // SSH terminal selection can contain display line wraps.
                let nonspace = paste.bytes().filter(|b| !b.is_ascii_whitespace()).count();
                if nonspace + text.len() > Offer::MAX_ENCODED_LEN {
                    return Err(Failure("route-offer-format"));
                }
                text.extend(paste.chars().filter(|c| !c.is_ascii_whitespace()));
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Enter => return Offer::decode(&text),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Err(Failure("peer-cancelled"));
                }
                KeyCode::Char(c) if c.is_ascii() && !c.is_ascii_whitespace() => {
                    if text.len() >= Offer::MAX_ENCODED_LEN {
                        return Err(Failure("route-offer-format"));
                    }
                    text.push(c);
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }
}

async fn connect() -> Result<()> {
    let offer = prompt_offer()?;
    let destination = offer.relay_candidate().unwrap_or(offer.direct_candidate());
    let bind = discover_bind(destination)?;
    let stop = CancellationToken::new();
    // Reserve the URL before dialing; never serve it until authenticated.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| Failure("application-loopback-bind"))?;
    let port = listener
        .local_addr()
        .map_err(|_| Failure("application-loopback-address"))?
        .port();
    if port == 18787 {
        return Err(Failure("application-origin-collision"));
    }
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let hostname = format!("thepeer-{}.localhost", hex::encode(nonce));
    let connected = tokio::select! {
        result = peer_route::connect(offer, bind, &stop) => result?,
        _ = stop_signal() => { stop.cancel(); return Ok(()); }
    };
    println!(
        "Connected ({:?}). Open http://{hostname}:{port} — Ctrl-C to disconnect.",
        connected.route()
    );
    let serving = connected.serve(listener, hostname, &stop);
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => result,
        _ = stop_signal() => { stop.cancel(); serving.await },
    }
}

async fn stop_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut term) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            tokio::select! { _ = signal::ctrl_c() => {}, _ = term.recv() => {} }
            return;
        }
    }
    let _ = signal::ctrl_c().await;
}

async fn share(args: &[String]) -> Result<()> {
    let (config, relay) = arguments(args)?.resolve()?;
    let mut ui = Surface::enter(config.target.to_string())?;
    let stop = CancellationToken::new();
    let preparing = async {
        match relay {
            Some(relay) => Helper::prepare_with_relay(config, relay, &stop).await,
            None => Helper::prepare(config, &stop).await,
        }
    };
    tokio::pin!(preparing);
    let ready = tokio::select! {
        result = &mut preparing => result,
        quit = ui.quit() => {
            stop.cancel();
            drop(ui);
            let cleanup = match preparing.await {
                Ok((helper, _)) => helper.shutdown().await,
                Err(Failure("peer-cancelled")) => Ok(()),
                Err(error) => Err(error),
            };
            return quit.and(cleanup);
        }
    };
    let (mut helper, offer) = ready?;
    let presentation = offer.encode().and_then(|text| {
        let code = QrCode::new(text.as_bytes()).map_err(|_| Failure("helper-qr"))?;
        ui.qr = Some(Zeroizing::new(
            code.render::<unicode::Dense1x2>().quiet_zone(true).build(),
        ));
        ui.offer = Some(text);
        ui.status = "Copy the connection string to your computer; run thepeer connect";
        Ok(())
    });
    drop(offer);
    let result = match presentation {
        Ok(()) => {
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    result = ui.quit() => break result,
                    result = helper.wait() => break result,
                    _ = tick.tick() => {
                        let progress = helper.progress();
                        if progress.admitted > 0 {
                            ui.qr = None;
                            ui.offer = None;
                            ui.status = if progress.authorized { "Connected" } else { "Authorizing selected peer" };
                        }
                    }
                }
            }
        }
        Err(error) => Err(error),
    };
    // Remove the capability surface before waiting for transport teardown.
    drop(ui);
    let cleanup = helper.shutdown().await;
    result.and(cleanup)
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), ResetColor, LeaveAlternateScreen, cursor::Show);
    }
}
struct Surface {
    target: String,
    qr: Option<Zeroizing<String>>,
    offer: Option<Zeroizing<String>>,
    status: &'static str,
    hidden: bool,
    show_qr: bool,
    previous: Option<((u16, u16), bool, bool, &'static str)>,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    _restore: Restore,
}
impl Surface {
    fn enter(target: String) -> Result<Self> {
        #[cfg(unix)]
        let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|_| Failure("helper-signal"))?;
        let restore = Restore;
        terminal::enable_raw_mode().map_err(|_| Failure("helper-terminal"))?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)
            .map_err(|_| Failure("helper-terminal"))?;
        Ok(Self {
            target,
            qr: None,
            offer: None,
            status: "Preparing (q to cancel)",
            hidden: false,
            show_qr: false,
            previous: None,
            #[cfg(unix)]
            terminate,
            _restore: restore,
        })
    }
    fn draw(&mut self) -> Result<()> {
        let size = terminal::size().map_err(|_| Failure("helper-terminal"))?;
        let view = (size, self.hidden, self.show_qr, self.status);
        if self.previous == Some(view) {
            return Ok(());
        }
        self.previous = Some(view);
        let mut out = io::stdout();
        execute!(
            out,
            ResetColor,
            Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            Print("thepeer | q / Esc / Ctrl-C: Stop | h: Hide | t: Text | r: QR"),
            cursor::MoveTo(0, 1),
            Print(self.status),
            cursor::MoveTo(0, 2),
            Print(format!("Fixed HTTP target: {}", self.target))
        )
        .map_err(|_| Failure("helper-terminal"))?;
        if !self.show_qr {
            if let Some(offer) = &self.offer {
                // Split only for display: paste strips whitespace before decoding.
                let width = usize::from(size.0).max(1);
                if self.hidden || usize::from(size.1) < 3 + offer.len().div_ceil(width) {
                    let message = if self.hidden {
                        "Connection string hidden"
                    } else {
                        "Connection string hidden: terminal too small"
                    };
                    execute!(out, cursor::MoveTo(0, 3), Print(message))
                        .map_err(|_| Failure("helper-terminal"))?;
                } else {
                    for (row, chunk) in offer.as_bytes().chunks(width).enumerate() {
                        let text =
                            std::str::from_utf8(chunk).map_err(|_| Failure("helper-terminal"))?;
                        execute!(out, cursor::MoveTo(0, row as u16 + 3), Print(text))
                            .map_err(|_| Failure("helper-terminal"))?;
                    }
                }
            }
            return Ok(());
        }
        if let Some(qr) = &self.qr {
            let width = qr
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0);
            let height = qr.lines().count();
            if self.hidden || width > size.0 as usize || height + 4 > size.1 as usize {
                let message = if self.hidden {
                    "QR hidden"
                } else {
                    "QR hidden: terminal too small"
                };
                execute!(out, cursor::MoveTo(0, 3), Print(message))
                    .map_err(|_| Failure("helper-terminal"))?;
            } else {
                execute!(
                    out,
                    SetForegroundColor(Color::Black),
                    SetBackgroundColor(Color::White)
                )
                .map_err(|_| Failure("helper-terminal"))?;
                for (row, line) in qr.lines().enumerate() {
                    execute!(out, cursor::MoveTo(0, row as u16 + 3), Print(line))
                        .map_err(|_| Failure("helper-terminal"))?;
                }
                execute!(out, ResetColor).map_err(|_| Failure("helper-terminal"))?;
            }
        }
        Ok(())
    }
    async fn quit(&mut self) -> Result<()> {
        let interrupt = tokio::signal::ctrl_c();
        tokio::pin!(interrupt);
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                result = &mut interrupt => return result.map_err(|_| Failure("helper-signal")),
                _ = async {
                    #[cfg(unix)] self.terminate.recv().await;
                    #[cfg(not(unix))] std::future::pending::<()>().await;
                } => return Ok(()),
                _ = tick.tick() => {
                    self.draw()?;
                    for _ in 0..32 {
                        // A zero timeout can skip input polling in the tty backend.
                        // At most 32 one-millisecond polls per tick bounds input work.
                        if !event::poll(Duration::from_millis(1)).map_err(|_| Failure("helper-terminal"))? { break; }
                        if let Event::Key(key) = event::read().map_err(|_| Failure("helper-terminal"))? {
                            if key.kind != KeyEventKind::Press { continue; }
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                                KeyCode::Char('h') => self.hidden = !self.hidden,
                                KeyCode::Char('r') => self.show_qr = true,
                                KeyCode::Char('t') => self.show_qr = false,
                                _ => {},
                            }
                        }
                    }
                }
            }
        }
    }
}
