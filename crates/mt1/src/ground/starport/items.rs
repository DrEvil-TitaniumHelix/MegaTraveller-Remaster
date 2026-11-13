use super::{Ground, objects::{self, Allocation}, stores};
pub use stores::buy_ammo as ammo;
pub fn allocate(g:&mut Ground,id:i16,store:bool)->i16 {
    objects::alloc(g,id,if store {Allocation::Store} else {Allocation::Empty})
}
