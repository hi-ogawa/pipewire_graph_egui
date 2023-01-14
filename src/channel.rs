#[derive(Copy, Clone, Debug)]
pub enum ChannelMessage {
    Noop,
    PipewireMainLoopReady,
    PipewireMainLoopStopRequest,
}
