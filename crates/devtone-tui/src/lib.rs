use devtone_core::palette::{self, rgb};
use devtone_core::{MusicParams, SpectrumSnap, StateFrame};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};

const BLOCKS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

pub fn bar_glyph(level: u8) -> char {
    if level >= 255 {
        '█'
    } else {
        BLOCKS[(level as usize * 8) / 256]
    }
}

pub fn header_line(state: &StateFrame, muted: bool) -> String {
    let run = if muted { "muted" } else { "running" };
    format!(
        "devtone  {} · {} · {} · {:.0} t/s · flow {:.2}  ● {run}",
        state.agent.as_str(),
        state.model.as_str(),
        state.lang.as_str(),
        state.out_tps,
        state.flow
    )
}

pub fn render_frame(
    frame: &mut Frame,
    state: &StateFrame,
    params: &MusicParams,
    snap: &SpectrumSnap,
    muted: bool,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(Line::from(Span::styled(
        header_line(state, muted),
        Style::default().fg(Color::Rgb(232, 230, 239)),
    )));
    frame.render_widget(header, chunks[0]);

    let bar_rows = (chunks[1].height as usize).clamp(1, 6);
    let mut lines = Vec::with_capacity(bar_rows);
    for row in 0..bar_rows {
        let mut spans = Vec::with_capacity(64);
        for (i, &level) in snap.bars.iter().enumerate() {
            let shown = if bar_rows == 1 {
                level
            } else {
                let band = 256 / bar_rows;
                let lo = (bar_rows - 1 - row) * band;
                let hi = lo + band;
                if (level as usize) <= lo {
                    0
                } else if (level as usize) >= hi {
                    255
                } else {
                    (((level as usize) - lo) * 255 / band.max(1)) as u8
                }
            };
            let (r, g, b) = rgb(palette::bar_color(i));
            spans.push(Span::styled(
                format!("{} ", bar_glyph(shown)),
                Style::default().fg(Color::Rgb(r, g, b)),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::NONE)), chunks[1]);

    let meters = format!(
        "vinyl {v}  hat {h}  pad {p}  cutoff {c:.1}k  bpm {bpm:.0}",
        v = meter(params.layers.vinyl),
        h = meter(params.layers.hat),
        p = meter(params.layers.pad),
        c = params.cutoff_hz / 1000.0,
        bpm = params.bpm
    );
    frame.render_widget(
        Paragraph::new(Span::styled(meters, Style::default().fg(Color::Rgb(168, 224, 200)))),
        chunks[2],
    );

    let footer = Paragraph::new(Span::styled(
        "space mute   q quit   n hide-notch   1-4 intensity",
        Style::default().fg(Color::Rgb(58, 61, 70)),
    ));
    frame.render_widget(footer, chunks[3]);
}

fn meter(v: f32) -> String {
    let n = (v.clamp(0.0, 1.0) * 8.0).round() as usize;
    let mut s = String::from("");
    for i in 0..8 {
        s.push(if i < n { '█' } else { '░' });
    }
    s
}

pub fn render<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &StateFrame,
    params: &MusicParams,
    snap: &SpectrumSnap,
    muted: bool,
) -> std::io::Result<()> {
    terminal.draw(|f| render_frame(f, state, params, snap, muted))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bar_glyph, header_line};
    use devtone_core::{AgentKind, Lang, StateFrame, TinyStr};

    #[test]
    fn bar_glyph_quantizes_to_blocks() {
        assert_eq!(bar_glyph(0), ' ');
        assert_eq!(bar_glyph(255), '█');
        assert_eq!(bar_glyph(128), '▄');
    }

    #[test]
    fn header_includes_agent_and_lang() {
        let mut s = StateFrame::default();
        s.agent = AgentKind::Pi;
        s.model = TinyStr::from_str_lossy("sonnet");
        s.lang = Lang::Rs;
        s.out_tps = 38.0;
        let h = header_line(&s, false);
        assert!(h.contains("pi"));
        assert!(h.contains("sonnet"));
        assert!(h.contains("rs"));
    }
}
