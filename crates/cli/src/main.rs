//! Target-only host command. Capabilities never leave the interactive QR surface.
use qrcode::{QrCode, render::unicode};
use ratatui::crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io::{self, IsTerminal},
    net::{SocketAddr, UdpSocket},
    process::ExitCode,
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use tunnel_core::{
    Failure,
    peer_route::{Helper, HelperConfig},
};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Failure>;

#[cfg(test)]
#[path = "../tests/internal/bind_tests.rs"]
mod bind_tests;

const USAGE: &str = "Usage: thepeer --target-port PORT [--bind IP:PORT] [--observer IP:PORT] [--relay IP:PORT]\nAutomatically select the local source address toward the participant (otherwise an IPv4 routing hint).\nOptional --bind overrides selection with an explicit unicast IP (port 0 allowed). Target is fixed localhost.\nParticipant options perform network bootstrap; no relay is operated by this helper.\nInteractive terminal required. Use the matching thepeer Android app.\nThe phone tries eligible Direct candidates then Relay; no coordinated punching.";

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
    target_port: u16,
    observer: Option<SocketAddr>,
    relay: Option<SocketAddr>,
}

fn arguments(args: &[String]) -> Result<Arguments> {
    let (mut bind, mut target, mut observer, mut relay) = (None, None, None, None);
    if args.len() > 8 || args.len() % 2 != 0 {
        return Err(Failure("helper-arguments"));
    }
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--bind" if bind.is_none() => bind = Some(address(&pair[1], true)?),
            "--target-port" if target.is_none() => {
                target = Some(
                    pair[1]
                        .parse::<u16>()
                        .ok()
                        .filter(|p| *p != 0)
                        .ok_or(Failure("helper-arguments"))?,
                );
            }
            "--observer" if observer.is_none() => observer = Some(address(&pair[1], false)?),
            "--relay" if relay.is_none() => relay = Some(address(&pair[1], false)?),
            _ => return Err(Failure("helper-arguments")),
        }
    }
    Ok(Arguments {
        bind,
        target_port: target.ok_or(Failure("helper-arguments"))?,
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
                target_port: self.target_port,
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
    let (config, relay) = arguments(&args)?.resolve()?;
    let mut ui = Surface::enter(config.target_port)?;
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
    // Drop plaintext immediately after building the ephemeral QR image.
    let encoded = offer.encode();
    let presentation = encoded.and_then(|text| {
        let code = QrCode::new(text.as_bytes()).map_err(|_| Failure("helper-qr"))?;
        ui.qr = Some(Zeroizing::new(
            code.render::<unicode::Dense1x2>().quiet_zone(true).build(),
        ));
        ui.status = "Scan with Android 0.6-direct-relay-preview";
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
    target_port: u16,
    qr: Option<Zeroizing<String>>,
    status: &'static str,
    hidden: bool,
    previous: Option<((u16, u16), bool, &'static str)>,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    _restore: Restore,
}
impl Surface {
    fn enter(target_port: u16) -> Result<Self> {
        #[cfg(unix)]
        let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|_| Failure("helper-signal"))?;
        let restore = Restore;
        terminal::enable_raw_mode().map_err(|_| Failure("helper-terminal"))?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)
            .map_err(|_| Failure("helper-terminal"))?;
        Ok(Self {
            target_port,
            qr: None,
            status: "Preparing (q to cancel)",
            hidden: false,
            previous: None,
            #[cfg(unix)]
            terminate,
            _restore: restore,
        })
    }
    fn draw(&mut self) -> Result<()> {
        let size = terminal::size().map_err(|_| Failure("helper-terminal"))?;
        let view = (size, self.hidden, self.status);
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
            Print("thepeer | q / Esc / Ctrl-C: Stop | h: Hide QR"),
            cursor::MoveTo(0, 1),
            Print(self.status),
            cursor::MoveTo(0, 2),
            Print(format!(
                "Fixed target: http://127.0.0.1:{}",
                self.target_port
            ))
        )
        .map_err(|_| Failure("helper-terminal"))?;
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
                                _ => {},
                            }
                        }
                    }
                }
            }
        }
    }
}
