use pipewire::{registry::GlobalObject, Properties};

#[derive(Debug)]
pub enum ChannelMessage {
    PipewireRegistryGlobal(ObjectWrapper),
    PipewireRegistryGlobalRemove(u32),
    PipewireMainLoopReady,
    PipewireMainLoopStopRequest,
    LinkCreate((String, String), (String, String)),
    LinkDestroy((String, String), (String, String)),
}

#[derive(Debug)]
pub struct ObjectWrapper(pub GlobalObject<Properties>);

// TODO: is it safe?
unsafe impl Send for ObjectWrapper {}
unsafe impl Sync for ObjectWrapper {}
