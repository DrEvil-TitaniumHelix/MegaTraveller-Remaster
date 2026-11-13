use super::{Charset, Context, Decision, Kind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicOption { pub id: String, pub label: String, pub enabled: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicRow { pub id: String, pub label: String, pub enabled: bool, pub price: u64, pub limit: u32, pub qty: u32, pub min_qty: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicList { pub rows: Vec<PublicRow>, pub current: usize, pub more: bool, pub money: u64 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicTextInput { pub max_len: usize, pub charset: Charset, pub width: Option<(u32, Vec<u8>)> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicTimeInput { pub max_ticks: u32, pub directions: Vec<String> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicDecision {
    pub id: String, pub kind: Kind, pub prompt: Option<String>, pub options: Vec<PublicOption>,
    pub list: Option<PublicList>, pub input: Option<PublicTextInput>, pub time: Option<PublicTimeInput>,
    pub default: Option<String>, pub context: Context, pub state: String,
    pub movement: Option<super::Movement>,
}
impl Decision {
    pub fn public(&self) -> PublicDecision {
        PublicDecision { id: self.id.clone(), kind: self.kind, prompt: self.prompt.clone(), default: self.default.clone(), context: self.context.clone(), state: self.state.clone(),
            movement: self.movement.clone(),
            options: self.options.iter().map(|o| PublicOption { id: o.id.clone(), label: o.label.clone(), enabled: o.enabled }).collect(),
            list: self.list.as_ref().map(|l| PublicList { current: l.current, more: l.more, money: l.money,
                rows: l.rows.iter().map(|r| PublicRow { id: r.id.clone(), label: r.label.clone(), enabled: r.enabled, price: r.price, limit: r.limit, qty: r.qty, min_qty: r.min_qty }).collect() }),
            input: self.input.as_ref().map(|i| PublicTextInput { max_len: i.max_len, charset: i.charset.clone(), width: i.width.clone() }),
            time: self.time.as_ref().map(|t| PublicTimeInput { max_ticks: t.max_ticks, directions: t.directions.keys().cloned().collect() }),
        }
    }
}
