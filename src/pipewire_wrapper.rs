use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result};

use crate::channel::ChannelMessage;
use pipewire::{prelude::ReadableDict, registry::GlobalObject, types::ObjectType, Properties};

pub struct PipewireWrapper {
    pub channel_sender: Sender<ChannelMessage>,
    pub channel_receiver: Receiver<ChannelMessage>,
    pub state: Arc<Mutex<PipewireState>>, // TODO: ui thread locks too much?
    thread_handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct PipewireState {
    pub error: bool, // TODO feedback error to UI e.g. via https://github.com/ItsEthra/egui-notify https://github.com/RegenJacob/egui_logger
    pub core_info: Option<String>,
    pub global_objects: BTreeMap<u32, GlobalObject<Properties>>,
}

// TODO: it's still non `Send` after `GlobalObject::to_owned` ??
unsafe impl Send for PipewireState {}

impl PipewireState {
    // factory names are supposed to be probed at runtime
    // evn though official tools depend on such convention e.g.
    //   "PipeWire:Interface:Link" => "link-factory"
    //    https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/792defde27e22673bd42b0584e875c78311e900b/src/tools/pw-cli.c#L1530
    fn get_factory_name(&self, object_type: pipewire::types::ObjectType) -> Option<&str> {
        self.global_objects
            .values()
            .filter(|object| object.type_ == pipewire::types::ObjectType::Factory)
            .flat_map(|object| {
                let props = object.props.as_ref()?;
                let factory_type_name = props.get(*pipewire::keys::FACTORY_TYPE_NAME)?;
                let factory_name = props.get(*pipewire::keys::FACTORY_NAME)?;
                if factory_type_name == object_type.to_str() {
                    Some(factory_name)
                } else {
                    None
                }
            })
            .next()
    }

    fn find_object_by_props<F: Fn(&Properties) -> bool>(
        &self,
        f: F,
    ) -> Option<&GlobalObject<Properties>> {
        self.global_objects
            .values()
            .filter(|object| object.props.as_ref().map_or(false, |props| f(props)))
            .next()
    }

    fn find_object_by_prop(&self, k: &str, v: &str) -> Option<&GlobalObject<Properties>> {
        self.find_object_by_props(|props| props.iter().find(|&kv| kv == (k, v)).is_some())
    }
}

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
            let registry = Rc::new(RefCell::new(core.get_registry().unwrap()));

            // channel message handler via `add_timer`
            // (`add_idle` looks too expensive)
            // (it would be probablly more efficient with `add_event` if we have one more thread to proxy message)
            let main_loop_weak = main_loop.downgrade();
            let core_ = core.clone();
            let state_ = state.clone();
            let registry_ = registry.clone();
            let timer_source = main_loop.add_timer(move |_| {
                let state = state_.lock().unwrap();
                while let Ok(message) = pw_receiver.try_recv() {
                    match message {
                        ChannelMessage::PipewireMainLoopStopRequest => {
                            main_loop_weak.upgrade().unwrap().quit();
                        }
                        ChannelMessage::LinkCreate(from, to) => {
                            #[rustfmt::skip]
                            let properties = || -> Option<Properties> {
                                use pipewire::keys::*;
                                let object_from = state.find_object_by_prop(from.0.as_str(), from.1.as_str())?;
                                let object_to = state.find_object_by_prop(to.0.as_str(), to.1.as_str())?;
                                let object_from_props = object_from.props.as_ref()?;
                                let object_to_props = object_to.props.as_ref()?;
                                let output_node = object_from_props.get(*NODE_ID)?;
                                let output_port = object_from.id.to_string();
                                let input_node = object_to_props.get(*NODE_ID)?;
                                let input_port = object_to.id.to_string();
                                Some(pipewire::properties! {
                                    *LINK_OUTPUT_NODE => output_node,
                                    *LINK_OUTPUT_PORT => output_port,
                                    *LINK_INPUT_NODE => input_node,
                                    *LINK_INPUT_PORT => input_port,
                                    *OBJECT_LINGER => "1" // otherwise the new object will removed immediately?
                                })
                            }();
                            dbg!(&properties);
                            if let Some(properties) = &properties {
                                // TODO: does Proxy need to be dropped manually?
                                let _link = core_
                                    .create_object::<pipewire::link::Link, _>(
                                        state
                                            .get_factory_name(pipewire::types::ObjectType::Link)
                                            .unwrap(),
                                        properties,
                                    )
                                    .unwrap();
                            } else {
                                tracing::error!("LinkCreate not found");
                            }
                        }
                        ChannelMessage::LinkDestroy(from, to) => {
                            #[rustfmt::skip]
                            let object_id = || -> Option<u32> {
                                use pipewire::keys::*;
                                let object_from = state.find_object_by_prop(from.0.as_str(), from.1.as_str())?;
                                let object_to =state.find_object_by_prop(to.0.as_str(), to.1.as_str())?;
                                let object_from_props = object_from.props.as_ref()?;
                                let object_to_props = object_to.props.as_ref()?;
                                let output_node = object_from_props.get(*NODE_ID)?;
                                let output_port = object_from.id.to_string();
                                let input_node = object_to_props.get(*NODE_ID)?;
                                let input_port = object_to.id.to_string();
                                let object = state.find_object_by_props(|props| {
                                    props.get(*LINK_OUTPUT_NODE) == Some(output_node) &&
                                    props.get(*LINK_OUTPUT_PORT) == Some(&output_port) &&
                                    props.get(*LINK_INPUT_NODE) == Some(input_node) &&
                                    props.get(*LINK_INPUT_PORT) == Some(&input_port)
                                })?;
                                Some(object.id)
                            }();
                            dbg!(&object_id);
                            if let Some(object_id) = object_id {
                                registry_.borrow().destroy_global(object_id).into_result().unwrap();
                            } else {
                                tracing::error!("LinkDestroy not found");
                            }
                        }
                        _ => {}
                    }
                }
            });
            timer_source
                .update_timer(
                    Some(Duration::from_millis(1)),
                    Some(Duration::from_millis(100)),
                )
                .into_result()
                .unwrap();

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
                    tracing::info!("core done");
                    dbg!((done_id, seq));
                })
                .error(|error_id, seq, res, message| {
                    tracing::error!("core error");
                    dbg!((error_id, seq, res, message));
                })
                .register();

            // registry event handler
            let pw_sender_1 = pw_sender.clone();
            let pw_sender_2 = pw_sender.clone();
            let state_1 = state.clone();
            let state_2 = state.clone();
            let _must_use = registry
                .borrow()
                .add_listener_local()
                .global(move |global_object| {
                    dbg!(global_object);
                    state_1
                        .lock()
                        .unwrap()
                        .global_objects
                        .insert(global_object.id, global_object.to_owned());
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

//
// object utilities
//

pub struct PipewireObject {}

impl PipewireObject {
    pub fn get_name(object: &GlobalObject<Properties>) -> Option<(&str, &str)> {
        use pipewire::keys::*;
        [
            *CLIENT_NAME,
            *CORE_NAME,
            *DEVICE_NAME,
            *FACTORY_NAME,
            *NODE_NAME,
            *MODULE_NAME,
            *APP_NAME,
            unsafe {
                std::ffi::CStr::from_bytes_with_nul_unchecked(pipewire::sys::PW_KEY_METADATA_NAME)
                    .to_str()
                    .unwrap()
            },
            *OBJECT_PATH,
            *PORT_ALIAS,
        ]
        .iter()
        .flat_map(|&k| {
            object
                .props
                .as_ref()
                .map(|prop| prop.get(k).map(|v| (k, v)))
                .flatten()
        })
        .next()
    }

    pub fn is_input(object: &GlobalObject<Properties>) -> bool {
        use pipewire::keys::*;
        object.type_ == ObjectType::Port
            && object
                .props
                .as_ref()
                .map(|prop| prop.get(*PORT_DIRECTION))
                .flatten()
                == Some("in")
    }

    pub fn is_output(object: &GlobalObject<Properties>) -> bool {
        use pipewire::keys::*;
        object.type_ == ObjectType::Port
            && object
                .props
                .as_ref()
                .map(|prop| prop.get(*PORT_DIRECTION))
                .flatten()
                == Some("out")
    }
}
