use std::{borrow::Cow, collections::HashMap};

use eframe::egui::{self, DragValue, TextStyle};
use egui_extras::{Size, TableBuilder};
use egui_node_graph::*;

use pipewire::types::ObjectType;
use serde::{Deserialize, Serialize};

use crate::{
    channel::ChannelMessage,
    pipewire_wrapper::{PipewireObject, PipewireWrapper},
};

// ========= First, define your user data types =============

/// The NodeData holds a custom data struct inside each node. It's useful to
/// store additional information that doesn't live in parameters. For this
/// example, the node data stores the template (i.e. the "type") of the node.
#[derive(Serialize, Deserialize)]
pub struct MyNodeData {
    template: MyNodeTemplate,
}

/// `DataType`s are what defines the possible range of connections when
/// attaching two ports together. The graph UI will make sure to not allow
/// attaching incompatible datatypes.
#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum MyDataType {
    Scalar,
    Vec2,
}

/// In the graph, input parameters can optionally have a constant value. This
/// value can be directly edited in a widget inside the node itself.
///
/// There will usually be a correspondence between DataTypes and ValueTypes. But
/// this library makes no attempt to check this consistency. For instance, it is
/// up to the user code in this example to make sure no parameter is created
/// with a DataType of Scalar and a ValueType of Vec2.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum MyValueType {
    Vec2 { value: egui::Vec2 },
    Scalar { value: f32 },
}

impl Default for MyValueType {
    fn default() -> Self {
        // NOTE: This is just a dummy `Default` implementation. The library
        // requires it to circumvent some internal borrow checker issues.
        Self::Scalar { value: 0.0 }
    }
}

impl MyValueType {
    /// Tries to downcast this value type to a vector
    pub fn try_to_vec2(self) -> anyhow::Result<egui::Vec2> {
        if let MyValueType::Vec2 { value } = self {
            Ok(value)
        } else {
            anyhow::bail!("Invalid cast from {:?} to vec2", self)
        }
    }

    /// Tries to downcast this value type to a scalar
    pub fn try_to_scalar(self) -> anyhow::Result<f32> {
        if let MyValueType::Scalar { value } = self {
            Ok(value)
        } else {
            anyhow::bail!("Invalid cast from {:?} to scalar", self)
        }
    }
}

/// NodeTemplate is a mechanism to define node templates. It's what the graph
/// will display in the "new node" popup. The user code needs to tell the
/// library how to convert a NodeTemplate into a Node.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum MyNodeTemplate {
    MakeVector,
    MakeScalar,
    AddScalar,
    SubtractScalar,
    VectorTimesScalar,
    AddVector,
    SubtractVector,
}

/// The response type is used to encode side-effects produced when drawing a
/// node in the graph. Most side-effects (creating new nodes, deleting existing
/// nodes, handling connections...) are already handled by the library, but this
/// mechanism allows creating additional side effects from user code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MyResponse {
    SetActiveNode(NodeId),
    ClearActiveNode,
}

/// The graph 'global' state. This state struct is passed around to the node and
/// parameter drawing callbacks. The contents of this struct are entirely up to
/// the user. For this example, we use it to keep track of the 'active' node.
#[derive(Default, Serialize, Deserialize)]
pub struct MyGraphState {
    pub active_node: Option<NodeId>,
}

// =========== Then, you need to implement some traits ============

// A trait for the data types, to tell the library how to display them
impl DataTypeTrait<MyGraphState> for MyDataType {
    fn data_type_color(&self, _user_state: &mut MyGraphState) -> egui::Color32 {
        match self {
            MyDataType::Scalar => egui::Color32::from_rgb(38, 109, 211),
            MyDataType::Vec2 => egui::Color32::from_rgb(238, 207, 109),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            MyDataType::Scalar => Cow::Borrowed("scalar"),
            MyDataType::Vec2 => Cow::Borrowed("2d vector"),
        }
    }
}

// A trait for the node kinds, which tells the library how to build new nodes
// from the templates in the node finder
impl NodeTemplateTrait for MyNodeTemplate {
    type NodeData = MyNodeData; // unit
    type DataType = MyDataType; // midi or audio
    type ValueType = MyValueType; // unit
    type UserState = MyGraphState; // unit

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        Cow::Borrowed(match self {
            MyNodeTemplate::MakeVector => "New vector",
            MyNodeTemplate::MakeScalar => "New scalar",
            MyNodeTemplate::AddScalar => "Scalar add",
            MyNodeTemplate::SubtractScalar => "Scalar subtract",
            MyNodeTemplate::AddVector => "Vector add",
            MyNodeTemplate::SubtractVector => "Vector subtract",
            MyNodeTemplate::VectorTimesScalar => "Vector times scalar",
        })
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        // It's okay to delegate this to node_finder_label if you don't want to
        // show different names in the node finder and the node itself.
        self.node_finder_label(user_state).into()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        MyNodeData { template: *self }
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
        node_id: NodeId,
    ) {
        // The nodes are created empty by default. This function needs to take
        // care of creating the desired inputs and outputs based on the template

        // We define some closures here to avoid boilerplate. Note that this is
        // entirely optional.
        let input_scalar = |graph: &mut MyGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                MyDataType::Scalar,
                MyValueType::Scalar { value: 0.0 },
                InputParamKind::ConnectionOrConstant,
                true,
            );
        };
        let input_vector = |graph: &mut MyGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                MyDataType::Vec2,
                MyValueType::Vec2 {
                    value: egui::vec2(0.0, 0.0),
                },
                InputParamKind::ConnectionOrConstant,
                true,
            );
        };

        let output_scalar = |graph: &mut MyGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), MyDataType::Scalar);
        };
        let output_vector = |graph: &mut MyGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), MyDataType::Vec2);
        };

        match self {
            MyNodeTemplate::AddScalar => {
                // The first input param doesn't use the closure so we can comment
                // it in more detail.
                graph.add_input_param(
                    node_id,
                    // This is the name of the parameter. Can be later used to
                    // retrieve the value. Parameter names should be unique.
                    "A".into(),
                    // The data type for this input. In this case, a scalar
                    MyDataType::Scalar,
                    // The value type for this input. We store zero as default
                    MyValueType::Scalar { value: 0.0 },
                    // The input parameter kind. This allows defining whether a
                    // parameter accepts input connections and/or an inline
                    // widget to set its value.
                    InputParamKind::ConnectionOrConstant,
                    true,
                );
                input_scalar(graph, "B");
                output_scalar(graph, "out");
            }
            MyNodeTemplate::SubtractScalar => {
                input_scalar(graph, "A");
                input_scalar(graph, "B");
                output_scalar(graph, "out");
            }
            MyNodeTemplate::VectorTimesScalar => {
                input_scalar(graph, "scalar");
                input_vector(graph, "vector");
                output_vector(graph, "out");
            }
            MyNodeTemplate::AddVector => {
                input_vector(graph, "v1");
                input_vector(graph, "v2");
                output_vector(graph, "out");
            }
            MyNodeTemplate::SubtractVector => {
                input_vector(graph, "v1");
                input_vector(graph, "v2");
                output_vector(graph, "out");
            }
            MyNodeTemplate::MakeVector => {
                input_scalar(graph, "x");
                input_scalar(graph, "y");
                output_vector(graph, "out");
            }
            MyNodeTemplate::MakeScalar => {
                input_scalar(graph, "value");
                output_scalar(graph, "out");
            }
        }
    }
}

pub struct AllMyNodeTemplates;
impl NodeTemplateIter for AllMyNodeTemplates {
    type Item = MyNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        // This function must return a list of node kinds, which the node finder
        // will use to display it to the user. Crates like strum can reduce the
        // boilerplate in enumerating all variants of an enum.
        vec![
            MyNodeTemplate::MakeScalar,
            MyNodeTemplate::MakeVector,
            MyNodeTemplate::AddScalar,
            MyNodeTemplate::SubtractScalar,
            MyNodeTemplate::AddVector,
            MyNodeTemplate::SubtractVector,
            MyNodeTemplate::VectorTimesScalar,
        ]
    }
}

impl WidgetValueTrait for MyValueType {
    type Response = MyResponse;
    type UserState = MyGraphState;
    type NodeData = MyNodeData;
    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: NodeId,
        ui: &mut egui::Ui,
        _user_state: &mut MyGraphState,
        _node_data: &MyNodeData,
    ) -> Vec<MyResponse> {
        // This trait is used to tell the library which UI to display for the
        // inline parameter widgets.
        match self {
            MyValueType::Vec2 { value } => {
                ui.label(param_name);
                ui.horizontal(|ui| {
                    ui.label("x");
                    ui.add(DragValue::new(&mut value.x));
                    ui.label("y");
                    ui.add(DragValue::new(&mut value.y));
                });
            }
            MyValueType::Scalar { value } => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(DragValue::new(value));
                });
            }
        }
        // This allows you to return your responses from the inline widgets.
        Vec::new()
    }
}

impl UserResponseTrait for MyResponse {}
impl NodeDataTrait for MyNodeData {
    type Response = MyResponse;
    type UserState = MyGraphState;
    type DataType = MyDataType;
    type ValueType = MyValueType;

    // TODO: titlebar layout looks broken without delete button
    fn can_delete(
        &self,
        _node_id: NodeId,
        _graph: &Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> bool {
        false
    }

    fn bottom_ui(
        &self,
        _ui: &mut egui::Ui,
        _node_id: NodeId,
        _graph: &Graph<MyNodeData, MyDataType, MyValueType>,
        _user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<MyResponse, MyNodeData>>
    where
        MyResponse: UserResponseTrait,
    {
        Default::default()
    }
}

type MyGraph = Graph<MyNodeData, MyDataType, MyValueType>;
type MyEditorState =
    GraphEditorState<MyNodeData, MyDataType, MyValueType, MyNodeTemplate, MyGraphState>;

pub struct NodeGraphExample {
    // The `GraphEditorState` is the top-level object. You "register" all your
    // custom types by specifying it as its generic parameters.
    state: MyEditorState,

    user_state: MyGraphState,

    pipewire_wrapper: PipewireWrapper,

    pipewire_id_to_node_id: HashMap<u32, NodeId>,

    extra_state: ExtraState,
}

#[derive(Default, Serialize, Deserialize)]
struct ExtraState {
    window_core: bool,
    window_object: bool,
    window_link: bool,
    link_from: Option<(String, String)>,
    link_to: Option<(String, String)>,
}

const PERSISTENCE_KEY: &str = env!("CARGO_PKG_NAME");

impl NodeGraphExample {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: Default::default(),
            user_state: Default::default(),
            pipewire_wrapper: PipewireWrapper::new(),
            pipewire_id_to_node_id: Default::default(),
            extra_state: cc
                .storage
                .and_then(|storage| eframe::get_value(storage, PERSISTENCE_KEY))
                .unwrap_or_default(),
        }
    }
}

impl eframe::App for NodeGraphExample {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.pipewire_wrapper.quit().unwrap();
    }

    /// If the persistence function is enabled,
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PERSISTENCE_KEY, &self.extra_state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(message) = self.pipewire_wrapper.channel_receiver.try_recv() {
            dbg!(&message);
            // TODO: is it guarnateed that registry events are order by Node -> Port -> Link?
            match &message {
                ChannelMessage::PipewireRegistryGlobal(object) => {
                    let object = &object.0;
                    match object.type_ {
                        ObjectType::Node => {
                            if let Some((_, name)) = PipewireObject::get_name(object) {
                                let node_id = self.state.graph.add_node(
                                    name.to_owned(),
                                    MyNodeData {
                                        template: MyNodeTemplate::AddScalar,
                                    },
                                    |_graph, _node_id| {
                                        // node_kind.build_node(graph, user_state, node_id)
                                    },
                                );
                                self.pipewire_id_to_node_id.insert(object.id, node_id);

                                self.state.node_order.push(node_id);

                                // pesudo random graph node position
                                let (x, y) = {
                                    use std::collections::hash_map::DefaultHasher;
                                    use std::hash::{Hash, Hasher};

                                    let mut hasher = DefaultHasher::new();
                                    name.hash(&mut hasher);
                                    let hash = hasher.finish();
                                    let (x, y) = ((hash >> 32) as f32, hash as u32 as f32);
                                    (x / (u32::MAX as f32) * 500.0, y / (u32::MAX as f32) * 500.0)
                                };
                                self.state.node_positions.insert(node_id, egui::pos2(x, y));
                            }
                        }
                        ObjectType::Port => {
                            || -> Option<()> {
                                let pipewire_id: u32 =
                                    PipewireObject::get_prop(object, *pipewire::keys::NODE_ID)?
                                        .parse()
                                        .ok()?;
                                let &node_id = self.pipewire_id_to_node_id.get(&pipewire_id)?;
                                let (_, port_name) = PipewireObject::get_name(object)?;
                                let port_direction = PipewireObject::get_prop(
                                    object,
                                    *pipewire::keys::PORT_DIRECTION,
                                )?;
                                match port_direction {
                                    "in" => {
                                        let _input_id = self.state.graph.add_input_param(
                                            node_id,
                                            port_name.to_string(),
                                            MyDataType::Scalar,
                                            MyValueType::Scalar { value: 0.0 },
                                            InputParamKind::ConnectionOnly,
                                            true,
                                        );
                                    }
                                    "out" => {
                                        let _output_id = self.state.graph.add_output_param(
                                            node_id,
                                            port_name.to_string(),
                                            MyDataType::Scalar,
                                        );
                                    }
                                    _ => {}
                                };
                                Some(())
                            }()
                            .unwrap_or_else(|| {
                                tracing::error!("invalid port {:?}", object);
                            });
                        }
                        ObjectType::Link => {
                            // self.state.graph.add_connection(output, input);
                            // self.state.graph.add_node(label, user_data, f)
                        }
                        _ => {}
                    }
                }
                ChannelMessage::PipewireRegistryGlobalRemove(_id) => {
                    // self.state.graph.remove_node(node_id);
                    // self.state.graph.remove_connection(input_id);
                }
                _ => {}
            }
        }

        //
        // menu bar
        //

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.toggle_value(&mut self.extra_state.window_core, "Core");
                ui.toggle_value(&mut self.extra_state.window_object, "Object");
                ui.toggle_value(&mut self.extra_state.window_link, "Link");
            });
        });

        //
        // core window
        //

        egui::Window::new("Core")
            .open(&mut self.extra_state.window_core)
            .default_width(500.0)
            .show(ctx, |ui| {
                if let Ok(state) = self.pipewire_wrapper.state.lock().as_deref() {
                    if let Some(core_info) = &state.core_info {
                        egui::ScrollArea::both().show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut core_info.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    } else {
                        ui.label("(initializing..)");
                    }
                } else {
                    ui.label("(error)");
                }
            });

        //
        // object window
        //

        egui::Window::new("Object")
            .open(&mut self.extra_state.window_object)
            .show(ctx, |ui| {
                let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
                egui::ScrollArea::both().show(ui, |ui| {
                    TableBuilder::new(ui)
                        .striped(true)
                        .column(Size::exact(20.0))
                        .column(Size::exact(80.0))
                        .column(Size::remainder())
                        .header(text_height, |mut header| {
                            header.col(|ui| {
                                ui.strong("ID");
                            });
                            header.col(|ui| {
                                ui.strong("Type");
                            });
                            header.col(|ui| {
                                ui.strong("Props");
                            });
                        })
                        .body(|mut body| {
                            let state = self.pipewire_wrapper.state.lock().unwrap();
                            for (_, object) in &state.global_objects {
                                body.row(text_height, |mut row| {
                                    row.col(|ui| {
                                        ui.label(object.id.to_string());
                                    });
                                    row.col(|ui| {
                                        ui.label(format!("{:?}", object.type_));
                                    });
                                    row.col(|ui| {
                                        let label = ui.label(
                                            PipewireObject::get_name(object)
                                                .map_or("--", |(_k, v)| v),
                                        );
                                        if let Some(props) = &object.props {
                                            label.on_hover_ui(|ui| {
                                                let props_str = format!("{:#?}", props);
                                                ui.add(
                                                    egui::TextEdit::multiline(
                                                        &mut props_str.as_str(),
                                                    )
                                                    .font(egui::TextStyle::Monospace)
                                                    .desired_width(f32::INFINITY),
                                                );
                                            });
                                        };
                                    });
                                });
                            }
                        });
                });
            });

        //
        // link create/destroy window
        //

        egui::Window::new("Link")
            .open(&mut self.extra_state.window_link)
            .show(ctx, |ui| {
                egui::Grid::new("link")
                    .num_columns(2)
                    .spacing([10.0, 5.0])
                    .show(ui, |ui| {
                        ui.label("From");
                        let selected = self.extra_state.link_from.clone();
                        egui::ComboBox::from_id_source("link-from")
                            .width(350.0)
                            .selected_text(
                                selected
                                    .as_ref()
                                    .map_or("".to_string(), |(_k, v)| v.clone()),
                            )
                            .show_ui(ui, |ui| {
                                let state = self.pipewire_wrapper.state.lock().unwrap();
                                for (_, object) in &state.global_objects {
                                    if PipewireObject::is_output(object) {
                                        if let Some((k, v)) = PipewireObject::get_name(object) {
                                            let mut response = ui.selectable_label(
                                                selected
                                                    .as_ref()
                                                    .map(|(k, v)| (k.as_str(), v.as_str()))
                                                    == Some((k, v)),
                                                v,
                                            );
                                            if response.clicked() {
                                                self.extra_state.link_from =
                                                    Some((k.to_owned(), v.to_owned()));
                                                response.mark_changed();
                                            }
                                        }
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label("To");
                        let selected = self.extra_state.link_to.clone();
                        egui::ComboBox::from_id_source("link-to")
                            .width(350.0)
                            .selected_text(
                                selected
                                    .as_ref()
                                    .map_or("".to_string(), |(_k, v)| v.clone()),
                            )
                            .show_ui(ui, |ui| {
                                let state = self.pipewire_wrapper.state.lock().unwrap();
                                for (_, object) in &state.global_objects {
                                    if PipewireObject::is_input(object) {
                                        if let Some((k, v)) = PipewireObject::get_name(object) {
                                            let mut response = ui.selectable_label(
                                                selected
                                                    .as_ref()
                                                    .map(|(k, v)| (k.as_str(), v.as_str()))
                                                    == Some((k, v)),
                                                v,
                                            );
                                            if response.clicked() {
                                                self.extra_state.link_to =
                                                    Some((k.to_owned(), v.to_owned()));
                                                response.mark_changed();
                                            }
                                        }
                                    }
                                }
                            });
                        ui.end_row();
                    });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ui.button("Create Link").clicked() {
                        match (&self.extra_state.link_from, &self.extra_state.link_to) {
                            (Some(from), Some(to)) => {
                                self.pipewire_wrapper
                                    .channel_sender
                                    .send(ChannelMessage::LinkCreate(from.clone(), to.clone()))
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                    if ui.button("Destroy Link").clicked() {
                        match (&self.extra_state.link_from, &self.extra_state.link_to) {
                            (Some(from), Some(to)) => {
                                self.pipewire_wrapper
                                    .channel_sender
                                    .send(ChannelMessage::LinkDestroy(from.clone(), to.clone()))
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                });
            });

        //
        // node graph
        //

        let graph_response = egui::CentralPanel::default()
            .show(ctx, |ui| {
                self.state
                    .draw_graph_editor(ui, AllMyNodeTemplates, &mut self.user_state)
            })
            .inner;
        for node_response in graph_response.node_responses {
            match node_response {
                NodeResponse::ConnectEventStarted(..) => {}
                NodeResponse::ConnectEventEnded { .. } => {}
                NodeResponse::User(user_event) => match user_event {
                    MyResponse::SetActiveNode(node) => self.user_state.active_node = Some(node),
                    MyResponse::ClearActiveNode => self.user_state.active_node = None,
                },
                _ => {}
            }
        }

        if let Some(node) = self.user_state.active_node {
            if self.state.graph.nodes.contains_key(node) {
                let text = match evaluate_node(&self.state.graph, node, &mut HashMap::new()) {
                    Ok(value) => format!("The result is: {:?}", value),
                    Err(err) => format!("Execution error: {}", err),
                };
                ctx.debug_painter().text(
                    egui::pos2(10.0, 35.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    TextStyle::Button.resolve(&ctx.style()),
                    egui::Color32::WHITE,
                );
            } else {
                self.user_state.active_node = None;
            }
        }
    }
}

type OutputsCache = HashMap<OutputId, MyValueType>;

/// Recursively evaluates all dependencies of this node, then evaluates the node itself.
pub fn evaluate_node(
    graph: &MyGraph,
    node_id: NodeId,
    outputs_cache: &mut OutputsCache,
) -> anyhow::Result<MyValueType> {
    // To solve a similar problem as creating node types above, we define an
    // Evaluator as a convenience. It may be overkill for this small example,
    // but something like this makes the code much more readable when the
    // number of nodes starts growing.

    struct Evaluator<'a> {
        graph: &'a MyGraph,
        outputs_cache: &'a mut OutputsCache,
        node_id: NodeId,
    }
    impl<'a> Evaluator<'a> {
        fn new(graph: &'a MyGraph, outputs_cache: &'a mut OutputsCache, node_id: NodeId) -> Self {
            Self {
                graph,
                outputs_cache,
                node_id,
            }
        }
        fn evaluate_input(&mut self, name: &str) -> anyhow::Result<MyValueType> {
            // Calling `evaluate_input` recursively evaluates other nodes in the
            // graph until the input value for a paramater has been computed.
            evaluate_input(self.graph, self.node_id, name, self.outputs_cache)
        }
        fn populate_output(
            &mut self,
            name: &str,
            value: MyValueType,
        ) -> anyhow::Result<MyValueType> {
            // After computing an output, we don't just return it, but we also
            // populate the outputs cache with it. This ensures the evaluation
            // only ever computes an output once.
            //
            // The return value of the function is the "final" output of the
            // node, the thing we want to get from the evaluation. The example
            // would be slightly more contrived when we had multiple output
            // values, as we would need to choose which of the outputs is the
            // one we want to return. Other outputs could be used as
            // intermediate values.
            //
            // Note that this is just one possible semantic interpretation of
            // the graphs, you can come up with your own evaluation semantics!
            populate_output(self.graph, self.outputs_cache, self.node_id, name, value)
        }
        fn input_vector(&mut self, name: &str) -> anyhow::Result<egui::Vec2> {
            self.evaluate_input(name)?.try_to_vec2()
        }
        fn input_scalar(&mut self, name: &str) -> anyhow::Result<f32> {
            self.evaluate_input(name)?.try_to_scalar()
        }
        fn output_vector(&mut self, name: &str, value: egui::Vec2) -> anyhow::Result<MyValueType> {
            self.populate_output(name, MyValueType::Vec2 { value })
        }
        fn output_scalar(&mut self, name: &str, value: f32) -> anyhow::Result<MyValueType> {
            self.populate_output(name, MyValueType::Scalar { value })
        }
    }

    let node = &graph[node_id];
    let mut evaluator = Evaluator::new(graph, outputs_cache, node_id);
    match node.user_data.template {
        MyNodeTemplate::AddScalar => {
            let a = evaluator.input_scalar("A")?;
            let b = evaluator.input_scalar("B")?;
            evaluator.output_scalar("out", a + b)
        }
        MyNodeTemplate::SubtractScalar => {
            let a = evaluator.input_scalar("A")?;
            let b = evaluator.input_scalar("B")?;
            evaluator.output_scalar("out", a - b)
        }
        MyNodeTemplate::VectorTimesScalar => {
            let scalar = evaluator.input_scalar("scalar")?;
            let vector = evaluator.input_vector("vector")?;
            evaluator.output_vector("out", vector * scalar)
        }
        MyNodeTemplate::AddVector => {
            let v1 = evaluator.input_vector("v1")?;
            let v2 = evaluator.input_vector("v2")?;
            evaluator.output_vector("out", v1 + v2)
        }
        MyNodeTemplate::SubtractVector => {
            let v1 = evaluator.input_vector("v1")?;
            let v2 = evaluator.input_vector("v2")?;
            evaluator.output_vector("out", v1 - v2)
        }
        MyNodeTemplate::MakeVector => {
            let x = evaluator.input_scalar("x")?;
            let y = evaluator.input_scalar("y")?;
            evaluator.output_vector("out", egui::vec2(x, y))
        }
        MyNodeTemplate::MakeScalar => {
            let value = evaluator.input_scalar("value")?;
            evaluator.output_scalar("out", value)
        }
    }
}

fn populate_output(
    graph: &MyGraph,
    outputs_cache: &mut OutputsCache,
    node_id: NodeId,
    param_name: &str,
    value: MyValueType,
) -> anyhow::Result<MyValueType> {
    let output_id = graph[node_id].get_output(param_name)?;
    outputs_cache.insert(output_id, value);
    Ok(value)
}

// Evaluates the input value of
fn evaluate_input(
    graph: &MyGraph,
    node_id: NodeId,
    param_name: &str,
    outputs_cache: &mut OutputsCache,
) -> anyhow::Result<MyValueType> {
    let input_id = graph[node_id].get_input(param_name)?;

    // The output of another node is connected.
    if let Some(other_output_id) = graph.connection(input_id) {
        // The value was already computed due to the evaluation of some other
        // node. We simply return value from the cache.
        if let Some(other_value) = outputs_cache.get(&other_output_id) {
            Ok(*other_value)
        }
        // This is the first time encountering this node, so we need to
        // recursively evaluate it.
        else {
            // Calling this will populate the cache
            evaluate_node(graph, graph[other_output_id].node, outputs_cache)?;

            // Now that we know the value is cached, return it
            Ok(*outputs_cache
                .get(&other_output_id)
                .expect("Cache should be populated"))
        }
    }
    // No existing connection, take the inline value instead.
    else {
        Ok(graph[input_id].value)
    }
}
