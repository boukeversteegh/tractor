// Visibility is exhaustive: every declaration carries either <private/>
// (implicit default) or <pub/> with optional restriction details.

fn private_fn() {}                      // -> <private/>
pub fn public_fn() {}                   // -> <pub/>
pub(crate) fn crate_fn() {}             // -> <pub><crate/></pub>
pub(super) fn super_fn() {}             // -> <pub><super/></pub>

struct PrivateStruct;                   // -> <private/>
pub struct PublicStruct;                // -> <pub/>

const PRIV: i32 = 1;                    // -> <private/>
pub const PUB: i32 = 2;                 // -> <pub/>
