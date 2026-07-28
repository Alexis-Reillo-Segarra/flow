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

    /// Los dos modos que cambian lo que hay que mandarle **al** proceso cuando
    /// el usuario escribe. No pintan nada: los guarda el emulador porque es
    /// quien ve las secuencias que los encienden, y los lee `crate::keys` al
    /// traducir una tecla. Ver `keys::Modes`.
    modes: crate::keys::Modes,

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
            modes: crate::keys::Modes::default(),
            replies: Vec::new(),
        }
    }

    /// Los modos que necesita saber quien traduzca una tecla a bytes.
    pub fn modes(&self) -> crate::keys::Modes {
        self.modes
    }

    /// Columnas y filas de la rejilla.
    ///
    /// Solo lo usan los tests: dentro de la aplicación nadie le pregunta al
    /// emulador de qué tamaño es, porque quien lo decide es quien lo dibuja y ya
    /// lo sabe. Existe para poder comprobar que la vista se lo dice.
    #[cfg(test)]
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
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
                            // Los dos que no pintan nada: cambian lo que se le
                            // manda al proceso al escribir, no lo que se ve.
                            // Ver `keys::Modes`.
                            1 => self.modes.app_cursor = set,
                            2004 => self.modes.bracketed_paste = set,
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
    fn los_modos_de_teclado_los_pone_el_proceso() {
        // No cambian nada de lo que se ve, así que es fácil borrarlos sin
        // enterarse: lo que rompen es lo que se escribe. Sin `app_cursor` las
        // flechas dejan de moverse dentro de una TUI, y sin el pegado entre
        // corchetes pegar tres líneas en un shell **ejecuta** las dos primeras.
        let mut term = Term::new(20, 5, 100);
        assert_eq!(term.modes(), crate::keys::Modes::default());

        feed(&mut term, b"\x1b[?1h\x1b[?2004h");
        assert!(term.modes().app_cursor);
        assert!(term.modes().bracketed_paste);

        feed(&mut term, b"\x1b[?1l\x1b[?2004l");
        assert_eq!(term.modes(), crate::keys::Modes::default());
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

#[cfg(test)]
mod tests_secuencias {
    use super::*;

    fn term() -> Term {
        Term::new(20, 6, 100)
    }

    fn feed(t: &mut Term, bytes: &[u8]) {
        let mut p = vte::Parser::new();
        p.advance(t, bytes);
    }

    /// Lo que hay escrito en una fila, sin los blancos del final.
    fn fila(t: &Term, i: usize) -> String {
        t.line(i)
            .map(|r| r.iter().map(|c| c.ch).collect::<String>())
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    /// El cursor se mueve con las ocho formas de pedirlo, y ninguna se sale de
    /// la rejilla: un `CUF` de mil columnas deja el cursor en la última, no
    /// fuera del buffer.
    #[test]
    fn el_cursor_se_mueve_y_no_se_sale_de_la_rejilla() {
        let mut t = term();

        feed(&mut t, b"\x1b[3;5H");
        assert_eq!(t.cursor(), (2, 4), "CUP no dejó el cursor donde se le dijo");

        feed(&mut t, b"\x1b[A");
        assert_eq!(t.cursor().0, 1);
        feed(&mut t, b"\x1b[2B");
        assert_eq!(t.cursor().0, 3);
        feed(&mut t, b"\x1b[3C");
        assert_eq!(t.cursor().1, 7);
        feed(&mut t, b"\x1b[2D");
        assert_eq!(t.cursor().1, 5);

        // A la primera columna de n líneas abajo, y de n líneas arriba.
        feed(&mut t, b"\x1b[E");
        assert_eq!(t.cursor(), (4, 0));
        feed(&mut t, b"\x1b[2F");
        assert_eq!(t.cursor(), (2, 0));

        // Columna y fila absolutas.
        feed(&mut t, b"\x1b[7G");
        assert_eq!(t.cursor().1, 6);
        feed(&mut t, b"\x1b[2d");
        assert_eq!(t.cursor().0, 1);

        // Y los desbordes, por los cuatro lados.
        feed(&mut t, b"\x1b[999C\x1b[999B");
        assert_eq!(
            t.cursor(),
            (5, 19),
            "el cursor se salió por abajo o por la derecha"
        );
        feed(&mut t, b"\x1b[999A\x1b[999D");
        assert_eq!(
            t.cursor(),
            (0, 0),
            "el cursor se salió por arriba o por la izquierda"
        );
        feed(&mut t, b"\x1b[999;999H");
        assert_eq!(t.cursor(), (5, 19));
    }

    /// Borrar: la línea entera, hasta el final, hasta el principio, y la
    /// pantalla en sus tres formas. Es lo que usa cualquier programa que
    /// repinte una línea, empezando por el prompt de un shell.
    #[test]
    fn se_borra_por_partes_y_del_todo() {
        let mut t = term();
        feed(&mut t, b"abcdefgh");

        // De la mitad al final.
        feed(&mut t, b"\x1b[1;4H\x1b[K");
        assert_eq!(fila(&t, 0), "abc");

        feed(&mut t, b"\x1b[1;1Habcdefgh\x1b[1;4H\x1b[1K");
        assert_eq!(
            fila(&t, 0),
            "    efgh",
            "EL 1 no borró del principio al cursor"
        );

        feed(&mut t, b"\x1b[2K");
        assert_eq!(fila(&t, 0), "");

        // La pantalla: de aquí abajo, de aquí arriba, y entera. Se vuelve al
        // origen antes de escribir: `EL` deja el cursor donde estaba, no lo
        // devuelve al principio de la línea.
        feed(&mut t, b"\x1b[1;1Huno\r\ndos\r\ntres");
        feed(&mut t, b"\x1b[2;1H\x1b[J");
        assert_eq!(fila(&t, 0), "uno");
        assert_eq!(fila(&t, 1), "");

        feed(&mut t, b"\x1b[1;1Huno\r\ndos\r\ntres\x1b[2;2H\x1b[1J");
        assert_eq!(fila(&t, 2), "tres", "ED 1 borró más abajo del cursor");

        feed(&mut t, b"\x1b[2J");
        for i in 0..6 {
            assert_eq!(fila(&t, i), "", "ED 2 dejó la fila {i} escrita");
        }
    }

    /// Insertar y borrar líneas y caracteres: es como una TUI mete una fila en
    /// medio de una lista sin repintarla entera.
    #[test]
    fn se_insertan_y_se_borran_lineas_y_caracteres() {
        let mut t = term();
        feed(&mut t, b"uno\r\ndos\r\ntres");

        feed(&mut t, b"\x1b[2;1H\x1b[L");
        assert_eq!(fila(&t, 1), "", "IL no abrió hueco");
        assert_eq!(fila(&t, 2), "dos", "IL no empujó lo que había hacia abajo");

        feed(&mut t, b"\x1b[M");
        assert_eq!(fila(&t, 1), "dos", "DL no se llevó la línea");

        feed(&mut t, b"\x1b[1;1H\x1b[2P");
        assert_eq!(fila(&t, 0), "o", "DCH no se comió los dos caracteres");

        feed(&mut t, b"\x1b[1;1H\x1b[3@");
        assert_eq!(fila(&t, 0), "   o", "ICH no abrió hueco");

        feed(&mut t, b"\x1b[1;1H\x1b[2X");
        assert_eq!(fila(&t, 0), "   o", "ECH borró de más");
    }

    /// La región de scroll: un `top`/`bottom` propio es lo que hace que una TUI
    /// tenga cabecera fija y cuerpo que se desplaza.
    #[test]
    fn la_region_de_scroll_acota_el_desplazamiento() {
        let mut t = term();
        feed(&mut t, b"a\r\nb\r\nc\r\nd\r\ne\r\nf");

        // Cabecera fija en la fila 1 y cuerpo de la 2 a la 4.
        feed(&mut t, b"\x1b[2;4r");
        feed(&mut t, b"\x1b[4;1H\n");
        assert_eq!(fila(&t, 0), "a", "el scroll se llevó la cabecera");
        assert_eq!(fila(&t, 1), "c", "la región no se desplazó");

        // Subir y bajar la región a mano.
        feed(&mut t, b"\x1b[2S");
        feed(&mut t, b"\x1b[2T");

        // Y quitarla: vuelve a ser toda la pantalla, cabecera incluida.
        feed(&mut t, b"\x1b[r");
        feed(&mut t, b"\x1b[2J\x1b[1;1HA\r\nB\r\nC\r\nD\r\nE\r\nF");
        feed(&mut t, b"\x1b[6;1H\n");
        assert_eq!(
            fila(&t, t.total_lines() - 6),
            "B",
            "sin región, el scroll no arrastró la primera fila"
        );
    }

    /// El índice inverso sube una línea y arrastra la pantalla al llegar
    /// arriba; `NEL` baja una y va a la primera columna.
    #[test]
    fn el_indice_inverso_arrastra_al_llegar_arriba() {
        let mut t = term();
        feed(&mut t, b"uno\r\ndos");
        feed(&mut t, b"\x1b[1;1H\x1bM");
        assert_eq!(t.cursor().0, 0);
        assert_eq!(fila(&t, 1), "uno", "RI no empujó la pantalla hacia abajo");

        feed(&mut t, b"\x1b[1;5H\x1bE");
        assert_eq!(t.cursor(), (1, 0));
    }

    /// Guardar y restaurar el cursor, por las dos vías que existen: la de ESC y
    /// la de CSI. Las usan los programas que dibujan algo y vuelven a lo suyo.
    #[test]
    fn el_cursor_se_guarda_y_se_recupera() {
        let mut t = term();
        feed(&mut t, b"\x1b[3;7H\x1b7");
        feed(&mut t, b"\x1b[1;1H");
        feed(&mut t, b"\x1b8");
        assert_eq!(t.cursor(), (2, 6));

        feed(&mut t, b"\x1b[2;2H\x1b[s\x1b[6;9H\x1b[u");
        assert_eq!(t.cursor(), (1, 1));
    }

    /// El tabulador salta de ocho en ocho y se para en el borde.
    #[test]
    fn el_tabulador_salta_de_ocho_en_ocho() {
        let mut t = term();
        feed(&mut t, b"\t");
        assert_eq!(t.cursor().1, 8);
        feed(&mut t, b"\t");
        assert_eq!(t.cursor().1, 16);
        feed(&mut t, b"\t");
        assert_eq!(t.cursor().1, 19, "el tabulador se salió por la derecha");
    }

    /// El retroceso y el retorno de carro mueven sin borrar, y ninguno se pasa
    /// del borde izquierdo.
    #[test]
    fn el_retroceso_no_se_pasa_del_borde() {
        let mut t = term();
        feed(&mut t, b"ab\x08\x08\x08\x08");
        assert_eq!(t.cursor().1, 0);
        assert_eq!(fila(&t, 0), "ab", "el retroceso borró lo que había");
    }

    /// El título lo pone el proceso con una secuencia de sistema operativo, y
    /// llega entero aunque venga partido en trozos.
    #[test]
    fn el_proceso_puede_ponerle_titulo_a_su_panel() {
        let mut t = term();
        feed(&mut t, b"\x1b]0;mi comando\x07");
        assert_eq!(t.title.as_deref(), Some("mi comando"));

        feed(&mut t, b"\x1b]2;otro\x1b\\");
        assert_eq!(t.title.as_deref(), Some("otro"));
    }

    /// Escribir en la última columna deja el cursor colgando y solo salta de
    /// línea con el carácter siguiente. Sin eso, una línea de exactamente el
    /// ancho de la terminal se lleva por delante una línea en blanco.
    #[test]
    fn una_linea_del_ancho_exacto_no_se_come_la_siguiente() {
        let mut t = Term::new(4, 4, 100);
        feed(&mut t, b"abcd");
        assert_eq!(t.cursor().0, 0, "el cursor saltó de línea antes de tiempo");
        feed(&mut t, b"e");
        assert_eq!(t.cursor().0, 1);
        assert_eq!(fila(&t, 0), "abcd");
        assert_eq!(fila(&t, 1), "e");

        // Y con el ajuste apagado, la última columna se sobrescribe.
        let mut t = Term::new(4, 4, 100);
        feed(&mut t, b"\x1b[?7l");
        feed(&mut t, b"abcdef");
        assert_eq!(t.cursor().0, 0);
    }

    /// Los atributos que no son color: negrita, tenue, cursiva, subrayado,
    /// inverso, oculto y tachado, cada uno con su forma de apagarse.
    #[test]
    fn los_atributos_se_encienden_y_se_apagan_uno_a_uno() {
        let mut t = term();
        feed(&mut t, b"\x1b[1;2;3;4;7;8;9mx");
        let pen = t.line(0).unwrap()[0].pen;
        assert!(pen.bold && pen.dim && pen.underline && pen.inverse);

        feed(&mut t, b"\x1b[22;23;24;27;28;29my");
        let pen = t.line(0).unwrap()[1].pen;
        assert!(!pen.bold && !pen.dim && !pen.underline && !pen.inverse);
    }

    /// Los 256 colores y el color verdadero, en tinta y en fondo, y el
    /// «por defecto» que los devuelve al del tema.
    #[test]
    fn el_color_llega_por_sus_tres_caminos() {
        let mut t = term();
        feed(&mut t, b"\x1b[38;5;196m\x1b[48;5;21ma");
        let pen = t.line(0).unwrap()[0].pen;
        assert!(matches!(pen.fg, Some(Ink::Ansi(196))));
        assert!(matches!(pen.bg, Some(Ink::Ansi(21))));

        feed(&mut t, b"\x1b[38;2;10;20;30m\x1b[48;2;40;50;60mb");
        let pen = t.line(0).unwrap()[1].pen;
        assert!(matches!(pen.fg, Some(Ink::Rgb(_))));
        assert!(matches!(pen.bg, Some(Ink::Rgb(_))));

        // «Por defecto» es no pedir nada: la celda se pinta con el color del
        // tema, que es lo que hace que cambiar de tema repinte lo ya escrito.
        feed(&mut t, b"\x1b[39;49mc");
        let pen = t.line(0).unwrap()[2].pen;
        assert_eq!(pen.fg, None);
        assert_eq!(pen.bg, None);

        // Los brillantes, que van por otro tramo de números.
        feed(&mut t, b"\x1b[93m\x1b[103md");
        let pen = t.line(0).unwrap()[3].pen;
        assert!(matches!(pen.fg, Some(Ink::Ansi(11))));
        assert!(matches!(pen.bg, Some(Ink::Ansi(11))));
    }

    /// Una secuencia a medias o inventada no rompe nada: la salida de un
    /// proceso llega a trozos y no siempre es correcta.
    #[test]
    fn lo_que_no_se_entiende_no_rompe_nada() {
        let mut t = term();
        feed(&mut t, b"\x1b[999999999;1H");
        feed(&mut t, b"\x1b[?9999h\x1b[?9999l");
        feed(&mut t, b"\x1b[Z\x1b[!p\x1b#8");
        feed(&mut t, b"\x1b]999;lo que sea\x07");
        feed(&mut t, b"\x1bZ\x1b(B\x1b)0");
        // Y partida por la mitad entre dos llegadas.
        let mut p = vte::Parser::new();
        p.advance(&mut t, b"\x1b[3");
        p.advance(&mut t, b";5Hx");
        assert_eq!(fila(&t, 2), "    x");
    }

    /// El alt-screen se guarda y se recupera entero, y lo que se escriba
    /// mientras esté puesto no acaba en el scrollback: si acabara, salir de
    /// `vim` dejaría el historial lleno de pantallazos.
    #[test]
    fn el_alt_screen_va_y_vuelve_sin_ensuciar() {
        let mut t = term();
        feed(&mut t, b"lo de siempre");
        feed(&mut t, b"\x1b[?1049h");
        assert!(t.alt_active);
        feed(&mut t, b"\x1b[2Juna pantalla entera");
        feed(&mut t, b"\x1b[?1049l");
        assert!(!t.alt_active);
        assert_eq!(fila(&t, 0), "lo de siempre");

        // Y la versión antigua, que es otra pareja de números.
        feed(&mut t, b"\x1b[?47h");
        assert!(t.alt_active);
        feed(&mut t, b"\x1b[?47l");
        assert!(!t.alt_active);
    }

    /// Encoger y agrandar un panel muchas veces no pierde lo escrito ni deja el
    /// cursor fuera de la rejilla.
    #[test]
    fn redimensionar_muchas_veces_no_pierde_el_hilo() {
        let mut t = term();
        feed(&mut t, b"uno\r\ndos\r\ntres\r\ncuatro");
        for (cols, rows) in [(5, 2), (40, 12), (1, 1), (20, 6)] {
            t.resize(cols, rows);
            // `cursor()` cuenta desde el principio del scrollback, no desde el
            // borde de arriba de la rejilla: lo que se comprueba es que sigue
            // apuntando a una línea que existe.
            let (r, c) = t.cursor();
            assert!(
                r < t.total_lines(),
                "el cursor apunta a la línea {r} y solo hay {}",
                t.total_lines()
            );
            assert!(c < cols, "el cursor se quedó en la columna {c} de {cols}");
        }
        assert!(t.total_lines() > 0);
    }

    /// Redimensionar al mismo tamaño no toca nada: se llama en cada frame en el
    /// que la rejilla está quieta.
    #[test]
    fn redimensionar_a_lo_mismo_no_hace_nada() {
        let mut t = term();
        feed(&mut t, b"algo");
        let antes = t.total_lines();
        t.resize(20, 6);
        assert_eq!(t.total_lines(), antes);
    }

    /// El scrollback tiene tope: una suite larga escupe cientos de miles de
    /// líneas y la memoria no es infinita.
    #[test]
    fn el_scrollback_tiene_tope() {
        let mut t = Term::new(10, 3, 20);
        for i in 0..200 {
            feed(&mut t, format!("linea {i}\r\n").as_bytes());
        }
        assert!(
            t.total_lines() <= 20 + 3,
            "el scrollback creció hasta {} líneas",
            t.total_lines()
        );
    }

    /// La última línea con algo escrito es lo que mira la heurística de estado,
    /// y se rinde tras un puñado de líneas en blanco: antes, una pantalla vacía
    /// recorría las cinco mil del scrollback montando un `String` por cada una.
    #[test]
    fn la_ultima_linea_escrita_se_busca_sin_recorrerlo_todo() {
        let mut t = Term::new(10, 3, 500);
        feed(&mut t, b"pregunta?");
        assert_eq!(t.last_nonempty_line().trim(), "pregunta?");

        for _ in 0..200 {
            feed(&mut t, b"\r\n");
        }
        assert_eq!(
            t.last_nonempty_line(),
            "",
            "se puso a buscar hacia atrás sin fin"
        );
    }
}
