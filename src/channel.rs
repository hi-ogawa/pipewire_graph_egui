#[derive(Clone, Debug)]
pub enum ChannelMessage {
    PipewireRegistryGlobal,
    PipewireRegistryGlobalRemove,
    PipewireMainLoopReady,
    PipewireMainLoopStopRequest,
    LinkCreate(String, String),  // input, output
    LinkDestroy(String, String), // input, output
}
