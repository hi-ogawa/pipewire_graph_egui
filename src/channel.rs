#[derive(Clone, Debug)]
pub enum ChannelMessage {
    PipewireRegistryGlobal,
    PipewireRegistryGlobalRemove,
    PipewireMainLoopReady,
    PipewireMainLoopStopRequest,
    LinkCreate((String, String), (String, String)),
    LinkDestroy((String, String), (String, String)),
}
