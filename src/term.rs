//! Emulador de terminal mínimo pero real.
//!
//! Un modelo por líneas no basta: los agentes de verdad (`claude`, `codex`)
//! repintan pantalla completa, mueven el cursor y usan alt-screen. Así que esto
//! mantiene una rejilla de celdas igual que un terminal, y las filas que se van
//! por arriba caen al scrollback. Con eso funcionan tanto `cargo test`
//! (salida en flujo) como una TUI a pantalla completa.
//!
//! No es un VT100 completo a propósito: no hay charsets alternativos, ni
//! caracteres de doble ancho, ni sixel. Cubre lo que un agente de terminal
//! realmente emite.

use std::collections::VecDeque;

use egui::Color32;
use vte::{Params, Perform};

use crate::theme;

/// La tinta que pide una celda: o un slot de la paleta, o un color exacto.
///
/// La distinción no es un capricho de tipos, es lo que hace que **cambiar de
/// tema repinte lo que ya está en pantalla**. Aquí antes se guardaba el
/// `Color32` ya resuelto, así que las 5000 líneas del scrollback se quedaban
/// pintadas con la paleta que estuviera puesta cuando llegaron, y al cambiar de
/// tema la salida vieja seguía siendo del tema viejo hasta que el proceso
/// escribiera otra vez.
///
/// Un proceso que pide "rojo" no pide `#f2696e`: pide el rojo de quien lo esté
/// mirando. El que sí pide un color exacto es el truecolor, y ese se guarda tal
/// cual —traducirlo a un slot sería inventarse lo que dijo—.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ink {
    /// Un índice de la paleta de 256, que incluye los 16 con nombre.
    Ansi(u8),
    /// Un color exacto, de `SGR 38;2;r;g;b`.
    Rgb(Color32),
}

impl Ink {
    /// El color que le toca hoy.
    pub fn color(self) -> Color32 {
        match self {
            Ink::Ansi(i) => theme::ansi256(i),
            Ink::Rgb(c) => c,
        }
    }
}

/// Atributos gráficos activos. Copiado en cada celda que se escribe.
///
/// El `Default` —sin color y sin atributos— es el estado tras `SGR 0`, y es
/// además el que tiene la inmensa mayoría de las celdas de una pantalla.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Pen {
    pub fg: Option<Ink>,
    pub bg: Option<Ink>,
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Pen {
    /// Color de primer plano ya resuelto contra el tema, aplicando inverso,
    /// negrita (sube a la variante brillante) y atenuado.
    pub fn fg_color(&self) -> Color32 {
        if self.inverse {
            return self.bg.map_or(theme::pal().bg, Ink::color);
        }
        let base = self.fg.map_or(theme::pal().text, Ink::color);
        if self.dim {
            base.gamma_multiply(0.55)
        } else {
            base
        }
    }

    /// Color de fondo resuelto, o `None` si es transparente.
    pub fn bg_color(&self) -> Option<Color32> {
        if self.inverse {
            Some(self.fg.map_or(theme::pal().text, Ink::color))
        } else {
            self.bg.map(Ink::color)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    pub ch: char,
    pub pen: Pen,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            pen: Pen::default(),
        }
    }
}

pub type Row = Vec<Cell>;

#[derive(Clone, Copy, Default)]
struct Cursor {
    row: usize,
    col: usize,
}

pub struct Term {
    pub cols: usize,
    pub rows: usize,

    grid: Vec<Row>,
    /// Rejilla principal guardada mientras el alt-screen está activo.
    saved_grid: Option<Vec<Row>>,
    scrollback: VecDeque<Row>,
    max_scrollback: usize,

    cur: Cursor,
    saved_cur: Cursor,
    pen: Pen,

    /// Región de scroll, ambos extremos inclusive.
    top: usize,
    bot: usize,

    /// DECAWM diferido: al escribir en la última columna el cursor se queda
    /// "colgando" y solo salta de línea con el siguiente carácter.
    wrap_pending: bool,
    autowrap: bool,

    pub alt_active: bool,
    pub title: Option<String>,

    /// Bytes que hay que devolverle al proceso.
    ///
    /// Un terminal no solo pinta: también contesta. ConPTY, nada más arrancar,
    /// pregunta dónde está el cursor con `ESC[6n` y **se queda bloqueado hasta
    /// que le respondes**: sin esto no llega ni un byte más de salida. El
    /// emulador no toca el PTY directamente, así que deja aquí las respuestas y
    /// `Agent::pump` las vacía hacia el proceso.
    pub replies: Vec<u8>,
}

impl Term {
    pub fn new(cols: usize, rows: usize, max_scrollback: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        Self {
            cols,
            rows,
            grid: vec![blank_row(cols); rows],
            saved_grid: None,
            scrollback: VecDeque::new(),
            max_scrollback,
            cur: Cursor::default(),
            saved_cur: Cursor::default(),
            pen: Pen::default(),
            top: 0,
            bot: rows - 1,
            wrap_pending: false,
            autowrap: true,
            alt_active: false,
            title: None,
            replies: Vec::new(),
        }
    }

    /// Número total de líneas direccionables: scrollback + rejilla visible.
    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.rows
    }

    /// Línea `i` contando el scrollback desde 0. Permite render virtualizado
    /// sin materializar todo el buffer.
    pub fn line(&self, i: usize) -> Option<&Row> {
        if i < self.scrollback.len() {
            self.scrollback.get(i)
        } else {
            self.grid.get(i - self.scrollback.len())
        }
    }

    /// Posición del cursor en coordenadas absolutas de `line()`.
    pub fn cursor(&self) -> (usize, usize) {
        (self.scrollback.len() + self.cur.row, self.cur.col)
    }

    /// Texto plano de la última línea con contenido. Lo usa la heurística de
    /// estado para detectar si el proceso está esperando una respuesta.
    ///
    /// La búsqueda se corta tras unas pocas líneas en blanco: si el proceso
    /// dejó media pantalla vacía por debajo del cursor, lo que haya más arriba
    /// ya no es "lo último que dijo". Sin ese tope, una pantalla en blanco
    /// recorrería las 5000 líneas del scrollback montando un `String` por cada
    /// una, y esto se llama desde el bucle de estado.
    pub fn last_nonempty_line(&self) -> String {
        const MAX_BLANKS: usize = 64;

        let mut blanks = 0;
        for i in (0..self.total_lines()).rev() {
            let Some(row) = self.line(i) else { continue };
            // Se mira si hay contenido antes de construir el `String`.
            let last = row.iter().rposition(|c| c.ch != ' ');
            match last {
                Some(end) => return row[..=end].iter().map(|c| c.ch).collect(),
                None => {
                    blanks += 1;
                    if blanks >= MAX_BLANKS {
                        break;
                    }
                }
            }
        }
        String::new()
    }

    /// Redimensiona la rejilla. Estrategia simple: se conserva el contenido
    /// alineado arriba-izquierda y se recorta lo que sobra. Reflow real de
    /// párrafos queda fuera de alcance.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }

        for row in &mut self.grid {
            row.resize(cols, Cell::default());
        }
        if rows < self.rows {
            let mut extra = self.rows - rows;
            // Lo que sobra se quita primero **por abajo**, mientras sean filas
            // en blanco por debajo del cursor: no tienen nada que preservar y
            // tirarlas deja el texto donde estaba.
            //
            // Importa más de lo que parece desde que la rejilla reparte la
            // pantalla entre varios paneles: un panel pequeño encoge mucho, y
            // archivar por arriba mandaba al scrollback la salida de un proceso
            // corto —que casi siempre cabe de sobra— dejando el panel en blanco
            // con el texto scrolleado fuera de la vista.
            while extra > 0 && self.grid.len() > self.cur.row + 1 {
                if !self.grid.last().is_some_and(is_blank) {
                    break;
                }
                self.grid.pop();
                extra -= 1;
            }
            // Solo cuando ya no queda hueco vacío se archiva por arriba, que es
            // la única forma de que el cursor siga cabiendo.
            for _ in 0..extra {
                if self.grid.is_empty() {
                    break;
                }
                let row = self.grid.remove(0);
                self.push_scrollback(row);
                self.cur.row = self.cur.row.saturating_sub(1);
            }
        }
        self.grid.resize(rows, blank_row(cols));

        self.cols = cols;
        self.rows = rows;
        self.top = 0;
        self.bot = rows - 1;
        self.cur.row = self.cur.row.min(rows - 1);
        self.cur.col = self.cur.col.min(cols - 1);
        self.wrap_pending = false;
    }

    // ─── Primitivas internas ──────────────────────────────────────────────

    fn push_scrollback(&mut self, mut row: Row) {
        // Recortar blancos finales: la mayoría de líneas son cortas y esto
        // baja el consumo del scrollback en un orden de magnitud.
        while row.last() == Some(&Cell::default()) {
            row.pop();
        }
        self.scrollback.push_back(row);
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.pop_front();
        }
    }

    /// Desplaza la región de scroll `n` líneas hacia arriba.
    fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            let row = self.grid.remove(self.top);
            // Solo se archiva lo que sale por el borde real de la pantalla
            // principal; en alt-screen o con región reducida se descarta.
            if self.top == 0 && !self.alt_active {
                self.push_scrollback(row);
            }
            self.grid.insert(self.bot, blank_row(self.cols));
        }
    }

    fn scroll_down(&mut self, n: usize) {
        for _ in 0..n {
            self.grid.remove(self.bot);
            self.grid.insert(self.top, blank_row(self.cols));
        }
    }

    fn linefeed(&mut self) {
        if self.cur.row == self.bot {
            self.scroll_up(1);
        } else if self.cur.row + 1 < self.rows {
            self.cur.row += 1;
        }
    }

    /// Índice reverso: sube una línea, desplazando si toca el borde superior.
    fn reverse_index(&mut self) {
        if self.cur.row == self.top {
            self.scroll_down(1);
        } else {
            self.cur.row = self.cur.row.saturating_sub(1);
        }
    }

    fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.grid.get_mut(row)?.get_mut(col)
    }

    fn erase_in_row(&mut self, row: usize, from: usize, to: usize) {
        let pen = Pen {
            // Al borrar se conserva el fondo activo, no el resto de atributos.
            bg: self.pen.bg,
            ..Pen::default()
        };
        if let Some(r) = self.grid.get_mut(row) {
            for c in from..to.min(r.len()) {
                r[c] = Cell { ch: ' ', pen };
            }
        }
    }

    fn enter_alt_screen(&mut self) {
        if self.alt_active {
            return;
        }
        self.saved_grid = Some(std::mem::replace(
            &mut self.grid,
            vec![blank_row(self.cols); self.rows],
        ));
        self.saved_cur = self.cur;
        self.cur = Cursor::default();
        self.alt_active = true;
        self.top = 0;
        self.bot = self.rows - 1;
    }

    fn leave_alt_screen(&mut self) {
        if !self.alt_active {
            return;
        }
        if let Some(mut prev) = self.saved_grid.take() {
            for row in &mut prev {
                row.resize(self.cols, Cell::default());
            }
            prev.resize(self.rows, blank_row(self.cols));
            self.grid = prev;
        }
        self.cur = self.saved_cur;
        self.cur.row = self.cur.row.min(self.rows - 1);
        self.cur.col = self.cur.col.min(self.cols - 1);
        self.alt_active = false;
        self.top = 0;
        self.bot = self.rows - 1;
    }

    /// Aplica una secuencia SGR (colores y atributos).
    fn sgr(&mut self, params: &Params) {
        let flat: Vec<&[u16]> = params.iter().collect();
        if flat.is_empty() {
            self.pen = Pen::default();
            return;
        }

        let mut i = 0;
        while i < flat.len() {
            let part = flat[i];
            let code = part.first().copied().unwrap_or(0);

            // Forma con subparámetros: 38:5:n o 38:2:r:g:b, todo en un slice.
            if part.len() > 1 && (code == 38 || code == 48 || code == 58) {
                let color = color_from_parts(&part[1..]);
                match code {
                    38 => self.pen.fg = color,
                    48 => self.pen.bg = color,
                    _ => {}
                }
                i += 1;
                continue;
            }

            match code {
                0 => self.pen = Pen::default(),
                1 => self.pen.bold = true,
                2 => self.pen.dim = true,
                4 => self.pen.underline = true,
                7 => self.pen.inverse = true,
                21 | 22 => {
                    self.pen.bold = false;
                    self.pen.dim = false;
                }
                24 => self.pen.underline = false,
                27 => self.pen.inverse = false,
                30..=37 => self.pen.fg = Some(Ink::Ansi(code as u8 - 30)),
                39 => self.pen.fg = None,
                40..=47 => self.pen.bg = Some(Ink::Ansi(code as u8 - 40)),
                49 => self.pen.bg = None,
                90..=97 => self.pen.fg = Some(Ink::Ansi(code as u8 - 90 + 8)),
                100..=107 => self.pen.bg = Some(Ink::Ansi(code as u8 - 100 + 8)),
                // Forma con parámetros separados por `;`: 38;5;n / 38;2;r;g;b.
                38 | 48 => {
                    let mode = flat.get(i + 1).and_then(|p| p.first()).copied();
                    let (color, consumed) = match mode {
                        Some(5) => (
                            flat.get(i + 2)
                                .and_then(|p| p.first())
                                .map(|v| Ink::Ansi(*v as u8)),
                            3,
                        ),
                        Some(2) => {
                            let get = |k: usize| {
                                flat.get(i + k)
                                    .and_then(|p| p.first())
                                    .copied()
                                    .unwrap_or(0) as u8
                            };
                            (Some(Ink::Rgb(Color32::from_rgb(get(2), get(3), get(4)))), 5)
                        }
                        _ => (None, 1),
                    };
                    if code == 38 {
                        self.pen.fg = color;
                    } else {
                        self.pen.bg = color;
                    }
                    i += consumed;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }
}

fn blank_row(cols: usize) -> Row {
    vec![Cell::default(); cols]
}

/// ¿Una fila sin nada que preservar? El fondo y los atributos cuentan: una fila
/// pintada de color no está en blanco aunque no tenga letras.
fn is_blank(row: &Row) -> bool {
    row.iter().all(|c| *c == Cell::default())
}

/// Decodifica `5;n` / `2;r;g;b` cuando vienen como subparámetros.
fn color_from_parts(parts: &[u16]) -> Option<Ink> {
    match parts.first()? {
        5 => parts.get(1).map(|v| Ink::Ansi(*v as u8)),
        2 => {
            // Algunas apps emiten 2:colorspace:r:g:b. Si hay 4 valores, el
            // primero es el espacio de color y se ignora.
            let rgb = if parts.len() >= 5 {
                &parts[2..5]
            } else {
                parts.get(1..4)?
            };
            Some(Ink::Rgb(Color32::from_rgb(
                rgb[0] as u8,
                rgb[1] as u8,
                rgb[2] as u8,
            )))
        }
        _ => None,
    }
}

impl Perform for Term {
    fn print(&mut self, c: char) {
        if self.wrap_pending {
            self.cur.col = 0;
            self.linefeed();
            self.wrap_pending = false;
        }
        let (row, col) = (self.cur.row, self.cur.col);
        let pen = self.pen;
        if let Some(cell) = self.cell_mut(row, col) {
            *cell = Cell { ch: c, pen };
        }
        if self.cur.col + 1 >= self.cols {
            if self.autowrap {
                self.wrap_pending = true;
            }
        } else {
            self.cur.col += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => {
                self.wrap_pending = false;
                self.linefeed();
            }
            b'\r' => {
                self.wrap_pending = false;
                self.cur.col = 0;
            }
            b'\t' => {
                self.wrap_pending = false;
                self.cur.col = ((self.cur.col / 8) + 1) * 8;
                self.cur.col = self.cur.col.min(self.cols - 1);
            }
            0x08 => {
                self.wrap_pending = false;
                self.cur.col = self.cur.col.saturating_sub(1);
            }
            _ => {} // BEL y compañía: sin efecto visual.
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        // Primer parámetro con valor por defecto 1 (el más común).
        let p1 = params
            .iter()
            .next()
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(0);
        let n = (p1 as usize).max(1);
        let private = intermediates.first() == Some(&b'?');

        match action {
            'A' => self.cur.row = self.cur.row.saturating_sub(n),
            'B' | 'e' => self.cur.row = (self.cur.row + n).min(self.rows - 1),
            'C' | 'a' => self.cur.col = (self.cur.col + n).min(self.cols - 1),
            'D' => self.cur.col = self.cur.col.saturating_sub(n),
            'E' => {
                self.cur.row = (self.cur.row + n).min(self.rows - 1);
                self.cur.col = 0;
            }
            'F' => {
                self.cur.row = self.cur.row.saturating_sub(n);
                self.cur.col = 0;
            }
            'G' | '`' => self.cur.col = (n - 1).min(self.cols - 1),
            'd' => self.cur.row = (n - 1).min(self.rows - 1),
            'H' | 'f' => {
                let mut it = params.iter();
                let row = it
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let col = it
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                self.cur.row = (row - 1).min(self.rows - 1);
                self.cur.col = (col - 1).min(self.cols - 1);
                self.wrap_pending = false;
            }
            'J' => {
                let (row, col, rows, cols) = (self.cur.row, self.cur.col, self.rows, self.cols);
                match p1 {
                    0 => {
                        self.erase_in_row(row, col, cols);
                        for r in row + 1..rows {
                            self.erase_in_row(r, 0, cols);
                        }
                    }
                    1 => {
                        for r in 0..row {
                            self.erase_in_row(r, 0, cols);
                        }
                        self.erase_in_row(row, 0, col + 1);
                    }
                    2 | 3 => {
                        for r in 0..rows {
                            self.erase_in_row(r, 0, cols);
                        }
                    }
                    _ => {}
                }
            }
            'K' => {
                let (row, col, cols) = (self.cur.row, self.cur.col, self.cols);
                match p1 {
                    0 => self.erase_in_row(row, col, cols),
                    1 => self.erase_in_row(row, 0, col + 1),
                    2 => self.erase_in_row(row, 0, cols),
                    _ => {}
                }
            }
            'L' => {
                // Insertar líneas en la región activa.
                let count = n.min(self.bot - self.cur.row + 1);
                for _ in 0..count {
                    self.grid.remove(self.bot);
                    self.grid.insert(self.cur.row, blank_row(self.cols));
                }
            }
            'M' => {
                let count = n.min(self.bot - self.cur.row + 1);
                for _ in 0..count {
                    self.grid.remove(self.cur.row);
                    self.grid.insert(self.bot, blank_row(self.cols));
                }
            }
            '@' => {
                let (row, col, cols) = (self.cur.row, self.cur.col, self.cols);
                if let Some(r) = self.grid.get_mut(row) {
                    for _ in 0..n.min(cols - col) {
                        r.insert(col, Cell::default());
                        r.truncate(cols);
                    }
                }
            }
            'P' => {
                let (row, col, cols) = (self.cur.row, self.cur.col, self.cols);
                if let Some(r) = self.grid.get_mut(row) {
                    for _ in 0..n.min(cols - col) {
                        r.remove(col);
                        r.push(Cell::default());
                    }
                }
            }
            'X' => {
                let (row, col, cols) = (self.cur.row, self.cur.col, self.cols);
                self.erase_in_row(row, col, (col + n).min(cols));
            }
            'S' => self.scroll_up(n),
            'T' => self.scroll_down(n),
            'r' => {
                let mut it = params.iter();
                let top = it
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize;
                let bot = it
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .filter(|v| *v > 0)
                    .map(|v| v as usize)
                    .unwrap_or(self.rows);
                self.top = (top - 1).min(self.rows - 1);
                self.bot = (bot - 1).min(self.rows - 1).max(self.top);
                self.cur = Cursor::default();
            }
            's' => self.saved_cur = self.cur,
            'u' => self.cur = self.saved_cur,
            'h' | 'l' => {
                let set = action == 'h';
                if private {
                    for p in params.iter() {
                        match p.first().copied().unwrap_or(0) {
                            7 => self.autowrap = set,
                            1047 | 1049 | 47 => {
                                if set {
                                    self.enter_alt_screen();
                                } else {
                                    self.leave_alt_screen();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            'm' => self.sgr(params),
            'n' => {
                // DSR. El caso 6 (posición del cursor) es el que bloquea a
                // ConPTY al arrancar, así que es obligatorio contestarlo.
                match p1 {
                    5 => self.replies.extend_from_slice(b"\x1b[0n"),
                    6 => {
                        let (row, col) = (self.cur.row + 1, self.cur.col + 1);
                        let prefix = if private { "\x1b[?" } else { "\x1b[" };
                        self.replies
                            .extend_from_slice(format!("{prefix}{row};{col}R").as_bytes());
                    }
                    _ => {}
                }
            }
            // Device Attributes: nos identificamos como un VT100 con opciones
            // avanzadas, que es lo que espera casi todo.
            'c' if !private => self.replies.extend_from_slice(b"\x1b[?1;2c"),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => self.reverse_index(),
            b'D' => self.linefeed(),
            b'E' => {
                self.cur.col = 0;
                self.linefeed();
            }
            b'7' => self.saved_cur = self.cur,
            b'8' => self.cur = self.saved_cur,
            b'c' => {
                self.grid = vec![blank_row(self.cols); self.rows];
                self.cur = Cursor::default();
                self.pen = Pen::default();
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 0 / 2: título de ventana. Muchos agentes lo usan para anunciar en
        // qué andan ("✳ Thinking…"), y eso sí interesa mostrarlo.
        //
        // Los shells, en cambio, lo ponen a su propia ruta (`C:\WINDOWS\
        // system32\cmd.exe`), que no dice nada y además tapa el comando real.
        // Se descartan los títulos que son sencillamente una ruta.
        if let (Some(kind), Some(value)) = (params.first(), params.get(1)) {
            if matches!(*kind, b"0" | b"2") {
                let text = String::from_utf8_lossy(value).trim().to_owned();
                let is_path = text.contains('\\') || text.contains('/') || text.ends_with(".exe");
                self.title = (!text.is_empty() && !is_path).then_some(text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vte::Parser;

    fn feed(term: &mut Term, bytes: &[u8]) {
        let mut parser = Parser::new();
        parser.advance(term, bytes);
    }

    #[test]
    fn encoger_un_panel_no_esconde_lo_que_ya_habia() {
        // Una salida corta en una rejilla grande: al repartir la pantalla entre
        // varios paneles, el terminal encoge mucho. Lo que sobra son las filas
        // en blanco de abajo, así que el texto tiene que seguir a la vista y el
        // scrollback vacío; si se archivara por arriba, el panel se quedaría en
        // blanco con la salida scrolleada fuera.
        let mut term = Term::new(40, 30, 100);
        feed(&mut term, b"cargo 1.97.1\r\n");
        term.resize(40, 8);

        assert_eq!(term.scrollback.len(), 0, "no debería haber archivado nada");
        assert_eq!(term.total_lines(), 8);
        assert!(text_of(&term, 0).starts_with("cargo 1.97.1"));
    }

    #[test]
    fn encoger_por_debajo_del_contenido_si_archiva() {
        // Cuando ya no queda hueco en blanco, no hay más remedio: se archiva
        // por arriba para que el cursor siga cabiendo.
        let mut term = Term::new(40, 6, 100);
        feed(&mut term, b"1\r\n2\r\n3\r\n4\r\n5\r\n");
        term.resize(40, 3);

        assert_eq!(term.scrollback.len(), 3);
        assert_eq!(text_of(&term, 0).trim_end(), "1");
        assert_eq!(text_of(&term, 3).trim_end(), "4");
        // Y el cursor sigue apuntando a la línea en la que estaba escribiendo.
        assert_eq!(term.cursor().0, 5);
    }

    fn text_of(term: &Term, line: usize) -> String {
        term.line(line)
            .map(|r| r.iter().map(|c| c.ch).collect::<String>())
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    /// Sin esta respuesta ConPTY se queda bloqueado nada más arrancar y el
    /// proceso no emite absolutamente nada. Es el fallo más caro de depurar de
    /// todo el emulador, así que queda clavado con un test.
    #[test]
    fn responde_a_la_consulta_de_cursor() {
        let mut term = Term::new(80, 24, 100);
        feed(&mut term, b"hola\x1b[6n");
        assert_eq!(term.replies, b"\x1b[1;5R");
    }

    #[test]
    fn responde_a_device_attributes() {
        let mut term = Term::new(80, 24, 100);
        feed(&mut term, b"\x1b[c");
        assert_eq!(term.replies, b"\x1b[?1;2c");
    }

    #[test]
    fn el_retorno_de_carro_sobrescribe() {
        // Las barras de progreso reescriben la línea con \r; si no se maneja,
        // el scrollback se llena de duplicados.
        let mut term = Term::new(20, 5, 100);
        feed(&mut term, b"cargando 10%\rcargando 99%");
        assert_eq!(text_of(&term, 0), "cargando 99%");
    }

    #[test]
    fn las_lineas_que_salen_van_al_scrollback() {
        let mut term = Term::new(20, 2, 100);
        feed(&mut term, b"uno\r\ndos\r\ntres");
        assert_eq!(term.total_lines(), 3);
        assert_eq!(text_of(&term, 0), "uno");
        assert_eq!(text_of(&term, 2), "tres");
    }

    #[test]
    fn el_sgr_pinta_y_se_reinicia() {
        let mut term = Term::new(20, 2, 100);
        feed(&mut term, b"\x1b[31mrojo\x1b[0mnormal");
        let row = term.line(0).unwrap();
        // Se guarda el slot, no el color: lo que el proceso pidió es "el rojo
        // del terminal", y cuál sea ese rojo lo decide el tema que esté puesto
        // cuando se dibuje.
        assert_eq!(row[0].pen.fg, Some(Ink::Ansi(1)));
        assert_eq!(row[0].pen.fg_color(), crate::theme::pal().ansi[1]);
        assert_eq!(row[4].pen.fg, None);
    }

    #[test]
    fn el_color_verdadero_llega_entero() {
        let mut term = Term::new(20, 2, 100);
        feed(&mut term, b"\x1b[38;2;18;52;86mx");
        // El truecolor sí se guarda tal cual: quien lo emite no pide un slot,
        // pide ese color, y ningún tema tiene derecho a cambiárselo.
        assert_eq!(
            term.line(0).unwrap()[0].pen.fg,
            Some(Ink::Rgb(egui::Color32::from_rgb(18, 52, 86)))
        );
    }

    #[test]
    fn cambiar_de_tema_repinta_lo_que_ya_estaba_escrito() {
        // Es lo que se gana guardando el slot: el scrollback entero se lee con
        // la paleta de ahora, no con la que hubiera cuando llegó el texto.
        let mut term = Term::new(20, 2, 100);
        feed(&mut term, b"\x1b[31mrojo");
        let pen = term.line(0).unwrap()[0].pen;

        let antes = crate::theme::active();
        let (a, b) = (0, crate::theme::themes().len() - 1);
        crate::theme::set_active(a);
        let rojo_a = pen.fg_color();
        crate::theme::set_active(b);
        let rojo_b = pen.fg_color();
        crate::theme::set_active(antes);

        assert_eq!(rojo_a, crate::theme::themes()[a].ansi[1]);
        assert_eq!(rojo_b, crate::theme::themes()[b].ansi[1]);
        assert_ne!(rojo_a, rojo_b, "los dos temas tenían el mismo rojo ANSI");
    }

    #[test]
    fn el_alt_screen_no_ensucia_el_scrollback() {
        let mut term = Term::new(20, 2, 100);
        feed(&mut term, b"historial\r\n");
        let before = term.total_lines();
        feed(&mut term, b"\x1b[?1049h");
        feed(&mut term, b"pantalla\r\ncompleta\r\nde\r\nuna\r\ntui\r\n");
        feed(&mut term, b"\x1b[?1049l");
        // Al volver, el historial previo sigue ahí y la TUI no dejó restos.
        assert_eq!(term.total_lines(), before);
        assert_eq!(text_of(&term, 0), "historial");
    }
}
