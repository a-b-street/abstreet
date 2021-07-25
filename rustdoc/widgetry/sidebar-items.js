initSidebarItems({"constant":[["DEFAULT_CORNER_RADIUS",""]],"enum":[["ClickOutcome","When an action happens through a button-like widget, what data is plumbed back?"],["ContentMode","Rules for how content should stretch to fill its bounds"],["ControlState",""],["CornerRounding",""],["DrawBaselayer","Before `State::draw` is called, draw something else."],["Event",""],["Fill",""],["Font",""],["HorizontalAlignment",""],["ImageSource","The visual"],["Key",""],["MultiKey",""],["Outcome","The result of a Panel handling an event"],["RewriteColor","A way to transform all colors in a GeomBatch."],["StackAlignment",""],["StackAxis",""],["Transition","When a state responds to an event, it can specify some way to manipulate the stack of states."],["UpdateType",""],["VerticalAlignment",""]],"fn":[["Line",""],["hotkeys",""],["lctrl",""],["run",""]],"macro":[["include_labeled_bytes","Like [`std::include_bytes!`], but also returns its argument, the relative path to the bytes"]],"mod":[["app_state","A widgetry application splits its state into two pieces: global shared state that lasts for the entire lifetime of the application, and a stack of smaller states, only one of which is active at a time. For example, imagine an application to view a map. The shared state would include the map and pre-rendered geometry for it. The individual states might start with a splash screen or menu to choose a map, then a map viewer, then maybe a state to drill down into pieces of the map."],["assets",""],["backend",""],["backend_glow",""],["backend_glow_native",""],["canvas",""],["color",""],["drawing",""],["event",""],["event_ctx",""],["geom",""],["input",""],["runner",""],["screen_geom",""],["style",""],["svg",""],["table",""],["text",""],["tools",""],["widgets",""]],"struct":[["Autocomplete",""],["ButtonBuilder",""],["ButtonStyle",""],["Cached","Store a cached key/value pair, only recalculating when the key changes."],["Canvas",""],["CanvasSettings",""],["Choice",""],["Color",""],["CompareTimes",""],["DrawWithTooltips",""],["Drawable","Geometry that’s been uploaded to the GPU once and can be quickly redrawn many times. Create by creating a `GeomBatch` and calling `ctx.upload(batch)`."],["EdgeInsets",""],["EventCtx",""],["FanChart",""],["Filler","Doesn’t do anything by itself, just used for widgetsing. Something else reaches in, asks for the ScreenRectangle to use."],["GeomBatch","A mutable builder for a group of colored polygons."],["GeomBatchStack","Similar to [`Widget::row`]/[`Widget::column`], but for [`GeomBatch`]s instead of [`Widget`]s, and follows a builder pattern"],["GfxCtx",""],["Image","A stylable UI component builder which presents vector graphics from an [`ImageSource`]."],["LinePlot",""],["LinearGradient",""],["Menu",""],["MultiButton",""],["Panel",""],["PersistentSplit",""],["PlotOptions",""],["Prerender",""],["ScatterPlot",""],["ScreenDims","ScreenDims is in units of logical pixels, as opposed to physical pixels."],["ScreenPt","ScreenPt is in units of logical pixels, as opposed to physical pixels."],["ScreenRectangle","ScreenRectangle is in units of logical pixels, as opposed to physical pixels."],["Series",""],["Settings","Customize how widgetry works. Most of these settings can’t be changed after starting."],["Slider",""],["Spinner",""],["Stash","An invisible widget that stores some arbitrary data on the Panel. Users of the panel can read and write the value. This is one method for “returning” data when a State completes."],["Style",""],["TabController",""],["Text",""],["TextBox",""],["TextSpan",""],["Texture",""],["Toggle",""],["UserInput",""],["Warper",""],["Widget",""],["WidgetOutput",""]],"trait":[["SharedAppState","Any data that should last the entire lifetime of the application should be stored in the struct implementing this trait."],["SimpleState","Many states fit a pattern of managing a single panel, handling mouseover events, and other interactions on the map. Implementing this instead of `State` reduces some boilerplate."],["State","A temporary state of an application. There’s a stack of these, with the most recent being the active one."],["TextExt",""],["WidgetImpl","Create a new widget by implementing this trait. You can instantiate your widget by calling `Widget::new(Box::new(instance of your new widget))`, which gives you the usual style options."]],"type":[["OutlineStyle",""]]});