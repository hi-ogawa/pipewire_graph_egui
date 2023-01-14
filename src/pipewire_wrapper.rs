use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

use anyhow::{Context, Result};

use crate::channel::ChannelMessage;

pub struct PipewireWrapper {
    pub channel_sender: Sender<ChannelMessage>,
    pub channel_receiver: Receiver<ChannelMessage>,
    thread_handle: Option<JoinHandle<()>>,
}

impl PipewireWrapper {
    pub fn new() -> Self {
        pipewire::init();

        // TODO: bidirection?
        let (ui_sender, pw_receiver) = mpsc::channel::<ChannelMessage>();
        let (pw_sender, ui_receiver) = mpsc::channel::<ChannelMessage>();

        let thread_handle = std::thread::spawn(move || {
            // TODO: error handling
            let main_loop = pipewire::MainLoop::new().unwrap();
            let context = pipewire::Context::new(&main_loop).unwrap();
            let core = context.connect(None).unwrap();
            let registry = core.get_registry().unwrap();

            // idle handler
            // TODO: is idle callback appropriate for handling message from UI?
            let main_loop_weak = main_loop.downgrade();
            let _must_use = main_loop.add_idle(true, move || {
                while let Ok(message) = pw_receiver.try_recv() {
                    match message {
                        ChannelMessage::PipewireMainLoopStopRequest => {
                            main_loop_weak.upgrade().unwrap().quit();
                        }
                        _ => {}
                    }
                }
            });

            // core event handler
            let _must_use = core
                .add_listener_local()
                .info(move |core_info| {
                    dbg!(core_info);
                    pw_sender
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
            let _must_use = registry
                .add_listener_local()
                .global(|global_object| {
                    dbg!(global_object);
                })
                .global_remove(|global_remove_id| {
                    dbg!(global_remove_id);
                })
                .register();

            main_loop.run(); // blocking
        });

        Self {
            channel_sender: ui_sender,
            channel_receiver: ui_receiver,
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
