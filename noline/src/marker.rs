pub trait SyncAsync {}

pub enum Sync {}
impl SyncAsync for Sync {}

pub enum Async {}
impl SyncAsync for Async {}
