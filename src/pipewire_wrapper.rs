use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
};

use anyhow::{Context, Result};

use crate::channel::ChannelMessage;
use pipewire::{prelude::ReadableDict, registry::GlobalObject, Properties};

pub struct PipewireWrapper {
    pub channel_sender: Sender<ChannelMessage>,
    pub channel_receiver: Receiver<ChannelMessage>,
    pub state: Arc<Mutex<PipewireState>>, // TODO: ui thread locks too much?
    thread_handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct PipewireState {
    pub error: bool, // TODO feedback error to UI e.g. via https://github.com/ItsEthra/egui-notify
    pub core_info: Option<String>,
    pub global_objects: HashMap<u32, GlobalObject<Properties>>,
}

// TODO: it's still non `Send` after `GlobalObject::to_owned` ??
unsafe impl Send for PipewireState {}

impl PipewireWrapper {
    pub fn new() -> Self {
        pipewire::init();

        // TODO: macro trick to reduce `xxx.clone()` patterns?

        let (ui_sender, pw_receiver) = mpsc::channel::<ChannelMessage>();
        let (pw_sender, ui_receiver) = mpsc::channel::<ChannelMessage>();

        let state = Arc::new(Mutex::new(PipewireState::default()));
        let state_clone = state.clone();

        let thread_handle = std::thread::spawn(move || {
            // TODO: error handling
            let main_loop = pipewire::MainLoop::new().unwrap();
            let context = pipewire::Context::new(&main_loop).unwrap();
            let core = context.connect(None).unwrap();
            let registry = core.get_registry().unwrap();

            // factory names are supposed to be probed at runtime
            // though even official tools depends on such convention as:
            //   "PipeWire:Interface:Link" => "link-factory"
            //   https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/792defde27e22673bd42b0584e875c78311e900b/src/tools/pw-cli.c#L1530
            let factory_name_map: Arc<Mutex<HashMap<String, String>>> = Default::default();

            // idle handler
            // TODO: is idle callback appropriate for handling message from UI?
            let main_loop_weak = main_loop.downgrade();
            let core_ = core.clone();
            let factory_name_map_ = factory_name_map.clone();
            let _must_use = main_loop.add_idle(true, move || {
                while let Ok(message) = pw_receiver.try_recv() {
                    match message {
                        ChannelMessage::PipewireMainLoopStopRequest => {
                            main_loop_weak.upgrade().unwrap().quit();
                        }
                        ChannelMessage::LinkCreate(_input, _output) => {
                            core_
                                .create_object::<pipewire::link::Link, _>(
                                    factory_name_map_.lock().unwrap()
                                        [&pipewire::types::ObjectType::Link.to_string()]
                                        .as_str(),
                                    &pipewire::properties! {
                                        *pipewire::keys::LINK_INPUT_NODE => "",
                                        *pipewire::keys::LINK_INPUT_PORT => "",
                                        *pipewire::keys::LINK_OUTPUT_NODE => "",
                                        *pipewire::keys::LINK_OUTPUT_PORT => "",
                                    },
                                )
                                .unwrap();
                        }
                        ChannelMessage::LinkDestroy(_, _) => {
                            core_
                                .create_object::<pipewire::link::Link, _>(
                                    factory_name_map_.lock().unwrap()
                                        [&pipewire::types::ObjectType::Link.to_string()]
                                        .as_str(),
                                    &pipewire::properties! {
                                        *pipewire::keys::LINK_INPUT_NODE => "",
                                        *pipewire::keys::LINK_INPUT_PORT => "",
                                        *pipewire::keys::LINK_OUTPUT_NODE => "",
                                        *pipewire::keys::LINK_OUTPUT_PORT => "",
                                    },
                                )
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            });

            // core event handler
            let state_ = state.clone();
            let pw_sender_1 = pw_sender.clone();
            let _must_use = core
                .add_listener_local()
                .info(move |core_info| {
                    state_.lock().unwrap().core_info = Some(format!("{:#?}", core_info));
                    pw_sender_1
                        .send(ChannelMessage::PipewireMainLoopReady)
                        .unwrap();
                })
                .done(|done_id, seq| {
                    dbg!(done_id, seq);
                })
                .error(|error_id, seq, res, message| {
                    dbg!(error_id, seq, res, message);
                })
                .register();

            // registry event handler
            let pw_sender_1 = pw_sender.clone();
            let pw_sender_2 = pw_sender.clone();
            let state_1 = state.clone();
            let state_2 = state.clone();
            let factory_name_map_ = factory_name_map.clone();
            let _must_use = registry
                .add_listener_local()
                .global(move |global_object| {
                    dbg!(global_object);
                    state_1
                        .lock()
                        .unwrap()
                        .global_objects
                        .insert(global_object.id, global_object.to_owned());
                    match global_object.type_ {
                        pipewire::types::ObjectType::Factory => {
                            let props = global_object.props.as_ref().unwrap();
                            factory_name_map_.lock().unwrap().insert(
                                props
                                    .get(*pipewire::keys::FACTORY_TYPE_NAME)
                                    .unwrap()
                                    .to_string(),
                                props
                                    .get(*pipewire::keys::FACTORY_NAME)
                                    .unwrap()
                                    .to_string(),
                            );
                        }
                        _ => {}
                    }
                    pw_sender_1
                        .send(ChannelMessage::PipewireRegistryGlobal)
                        .unwrap();
                })
                .global_remove(move |global_remove_id| {
                    dbg!(global_remove_id);
                    state_2
                        .lock()
                        .unwrap()
                        .global_objects
                        .remove(&global_remove_id);
                    pw_sender_2
                        .send(ChannelMessage::PipewireRegistryGlobalRemove)
                        .unwrap();
                })
                .register();

            main_loop.run(); // blocking
        });

        Self {
            channel_sender: ui_sender,
            channel_receiver: ui_receiver,
            state: state_clone,
            thread_handle: Some(thread_handle),
        }
    }

    pub fn quit(&mut self) -> Result<()> {
        self.channel_sender
            .send(ChannelMessage::PipewireMainLoopStopRequest)?;

        // TODO: handle with timeout in case of pipewire thread halting?
        self.thread_handle
            .take()
            .context("invalid thread_handle")?
            .join()
            .ok()
            .context("join error")?;

        unsafe {
            pipewire::deinit();
        }
        Ok(())
    }
}
