//! Shared business work; all renderer variants execute the same edit sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quote {
    pub index: usize,
    pub price: i64,
    pub quantity: i64,
    pub up: bool,
    pub selected: bool,
}
impl Quote {
    pub fn symbol(self) -> String {
        format!("ASSET {:05}", self.index + 1)
    }
    pub fn price(self) -> String {
        money(self.price)
    }
    pub fn quantity(self) -> String {
        self.quantity.to_string()
    }
    pub fn value(self) -> i64 {
        self.price * self.quantity
    }
    pub fn amount(self) -> String {
        money(self.value())
    }
    pub fn class(self) -> String {
        format!(
            "quote {}{}",
            if self.up { "up" } else { "down" },
            if self.selected { " selected" } else { "" }
        )
    }
}
pub fn money(value: i64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}
#[derive(Default)]
pub struct Market {
    pub quotes: Vec<Quote>,
    pub total: i64,
    step: usize,
    selection_step: usize,
    selected: Option<usize>,
}
impl Market {
    pub fn mount(&mut self, n: usize) -> Vec<Quote> {
        *self = Self::default();
        self.quotes = (0..n)
            .map(|index| Quote {
                index,
                price: 10_000 + (index * 37 % 20_000) as i64,
                quantity: (index % 99 + 1) as i64,
                up: true,
                selected: false,
            })
            .collect();
        self.total = self.quotes.iter().map(|q| q.value()).sum();
        self.quotes.clone()
    }
    pub fn tick(&mut self, count: usize, repeats: usize) -> Vec<Quote> {
        let n = self.quotes.len();
        let mut edits = Vec::with_capacity(count.min(n) * repeats);
        if n == 0 {
            return edits;
        }
        for _ in 0..repeats {
            self.step += 1;
            for offset in 0..count.min(n) {
                let index = (self.step * 73 + offset) % n;
                let q = &mut self.quotes[index];
                self.total -= q.value();
                q.price = 10_000
                    + (index * 37 % 20_000) as i64
                    + ((self.step * 17 + index * 13) % 1999) as i64;
                q.quantity = ((self.step * 3 + index * 7) % 100 + 1) as i64;
                q.up = (self.step + index).is_multiple_of(2);
                self.total += q.value();
                edits.push(*q);
            }
        }
        edits
    }
    pub fn select(&mut self) -> Vec<Quote> {
        if self.quotes.is_empty() {
            return Vec::new();
        }
        self.selection_step += 1;
        let index = (self.selection_step * 97) % self.quotes.len();
        let mut edits = Vec::with_capacity(2);
        if let Some(old) = self.selected.replace(index) {
            self.quotes[old].selected = false;
            edits.push(self.quotes[old]);
        }
        self.quotes[index].selected = true;
        edits.push(self.quotes[index]);
        edits
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
