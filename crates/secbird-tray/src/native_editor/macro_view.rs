//! Local compatible macro library and bounded, explicitly saved definition drafts.
use super::*;
use secbird_core::{Store,profile_compat::macro_library::{self as library,Change,Entry,Library}};
use std::{collections::{BTreeMap,VecDeque},io::Read};
use windows_sys::Win32::{Security::Cryptography::{BCryptGenRandom,BCRYPT_USE_SYSTEM_PREFERRED_RNG},UI::Controls::Dialogs::*};
use windows_sys::Win32::UI::Controls::EM_SETREADONLY;
use library::input_events::{self,Input,Gesture};
pub(super) const ID:&str="module:macro";
pub(super) const BROWSE:u32=WM_APP+172;
pub(super) const INPUT_MENU:u32=WM_APP+174;
#[cfg(test)] pub(super) const TEST:u32=WM_APP+173;
const NAME:u16=2300;const DELAY:u16=2301;

struct Draft {
    entry:Entry,
    base_revision:String,
    undo:VecDeque<String>,
    redo:VecDeque<String>,
}
impl Draft {
    fn new(entry:Entry)->Self {Self{base_revision:entry.revision.clone(),entry,undo:VecDeque::new(),redo:VecDeque::new()}}
    fn dirty(&self)->bool {self.entry.revision!=self.base_revision}
    fn apply(&mut self,change:Change)->Result<(),String> {
        let next=library::edit_definition(&self.entry.encoded_record,change).map_err(|e|e.to_string())?;
        if next==self.entry.encoded_record{return Ok(());}
        let mut described=library::describe(&next).map_err(|e|e.to_string())?;described.referenced=self.entry.referenced;
        self.undo.push_back(self.entry.encoded_record.to_string());self.redo.clear();self.entry=described;self.trim();Ok(())
    }
    fn trim(&mut self) {while self.undo.len()+self.redo.len()>20||self.undo.iter().chain(&self.redo).map(String::len).sum::<usize>()>4*1024*1024 {
        if self.undo.pop_front().is_none(){self.redo.pop_front();}
    }}
    fn history(&mut self,redo:bool)->Result<(),String> {
        let queue=if redo{&self.redo}else{&self.undo};let Some(text)=queue.back() else{return Ok(());};
        let value:Value=serde_json::from_str(text).map_err(|e|e.to_string())?;let mut next=library::describe(&value).map_err(|e|e.to_string())?;next.referenced=self.entry.referenced;
        let current=self.entry.encoded_record.to_string();
        if redo {self.redo.pop_back();self.undo.push_back(current);}else{self.undo.pop_back();self.redo.push_back(current);}
        self.entry=next;self.trim();Ok(())
    }
}
#[derive(Clone)]
pub enum Request {Read,Change(Change),Import(PathBuf)}
impl Request {pub fn mutates(&self)->bool {!matches!(self,Self::Read)}}
pub struct Data {library:Library}
#[derive(Clone)]
pub enum Control {Open,List,Select(String),New,Duplicate,Delete,Import,Save,Discard,Undo,Redo,Event(usize),AddDelay,ApplyDelay,DeleteEvent,Move(i8),InputMenu,GestureMenu,ChooseInput(Input),ChooseGesture(Gesture),AddInput}
#[derive(Default)]
pub(super) struct View {
    pub used:bool,
    pub busy:bool,
    data:Option<Data>,
    selected:Option<String>,
    pending_selection:Option<String>,
    draft:Option<Draft>,
    name:String,
    delay:String,
    event:Option<usize>,
    picker:bool,
    picker_scroll:usize,
    scroll:usize,
    status:String,
    failed:bool,
    input:Option<Input>,
    gesture:Option<Gesture>,
    pub popup:bool,
}
pub(super) fn active(state:&State)->bool {state.selected.as_deref()==Some(ID)}
fn identity()->Result<String,String> {
    let mut bytes=[0u8;16];let status=unsafe{BCryptGenRandom(null_mut(),bytes.as_mut_ptr(),16,BCRYPT_USE_SYSTEM_PREFERRED_RNG)};
    if status<0{return Err("Could not create a macro identity".into());}bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;
    let s:String=bytes.iter().map(|v|format!("{v:02x}")).collect();Ok(format!("{}-{}-{}-{}-{}",&s[..8],&s[8..12],&s[12..16],&s[16..20],&s[20..]))
}
pub(super) fn work(request:Request,read_only:bool)->Result<Data,String> {
    if read_only&&request.mutates(){return Err("Read-only mode refuses macro changes".into());}
    let mut store=Store::load(&secbird_core::config_dir().join("profiles.json")).map_err(|e|e.to_string())?;
    let change=match request {
        Request::Read=>None,Request::Change(change)=>Some(change),Request::Import(path)=>{
            let file=std::fs::File::open(path).map_err(|e|e.to_string())?;if !file.metadata().map_err(|e|e.to_string())?.is_file(){return Err("Choose a regular profile export file".into());}
            let mut bytes=vec![];file.take(2*1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;if bytes.len()>2*1024*1024{return Err("Macro import exceeds2MiB".into());}Some(Change::Import(bytes))
        }
    };
    if let Some(change)=change {store=library::edit(&store,change).map_err(|e|e.to_string())?;store.save().map_err(|e|e.to_string())?;}
    Ok(Data{library:library::read(&store).map_err(|e|e.to_string())?})
}
fn fixed_delay(entry:&Entry,index:usize)->Option<u32> {
    let event=entry.events.get(index)?;if event["Type"]!=0||event["Number"].is_object(){return None;}
    let value=event["Number"].as_f64().or_else(||event["Number"].as_str()?.parse().ok())?;
    let ms=(value*1000.).round();(ms.is_finite()&&ms>=0.&&ms<=3_600_000.).then_some(ms as u32)
}
impl View {
    fn dirty(&self)->bool {self.draft.as_ref().is_some_and(|d|d.dirty()||self.name!=d.entry.name||self.event.and_then(|i|fixed_delay(&d.entry,i)).is_some_and(|ms|self.delay.trim().parse::<u32>().ok()!=Some(ms)))}
    fn select(&mut self,id:Option<String>) {
        self.selected=id;self.event=None;self.scroll=0;self.picker=false;
        self.draft=self.data.as_ref().and_then(|d|d.library.entries.iter().find(|r|Some(&r.guid)==self.selected.as_ref())).cloned().map(Draft::new);
        self.name=self.draft.as_ref().map_or(String::new(),|d|d.entry.name.clone());self.delay="1000".into();
    }
}
pub(super) fn can_leave(state:&mut State)->bool {
    if !active(state)||!state.macros.dirty(){return true;}
    state.macros.status="Save or discard the macro draft before leaving this page.".into();state.macros.failed=true;state.exit_tray=None;state.closing=false;unsafe{InvalidateRect(state.hwnd,null(),0);}false
}
pub(super) unsafe fn request(state:&mut State,request:Request) {
    if state.macros.busy||((state.read_only_smoke||state.restricted_actions)&&request.mutates()){return;}
    let before=(state.pending,state.pending_jobs.len());state.queue(Job::MacroLibrary(request));state.macros.busy=before!=(state.pending,state.pending_jobs.len());
}
pub(super) unsafe fn receive(state:&mut State,result:Result<Data,String>) {
    state.macros.busy=false;
    match result {
        Ok(data)=>{
            let selected=state.macros.pending_selection.take().or_else(||state.macros.selected.clone()).filter(|id|data.library.entries.iter().any(|e|e.guid==*id)).or_else(||data.library.entries.first().map(|e|e.guid.clone()));
            let saved=data.library.entries.iter().find(|entry|Some(&entry.guid)==selected.as_ref());
            let keep_history=state.macros.draft.as_ref().is_some_and(|d|saved.is_some_and(|entry|entry.guid==d.entry.guid&&entry.revision==d.entry.revision));
            if keep_history {let d=state.macros.draft.as_mut().unwrap();let saved=saved.unwrap();d.base_revision=saved.revision.clone();d.entry.referenced=saved.referenced;state.macros.selected=selected;state.macros.data=Some(data);}else{state.macros.data=Some(data);state.macros.select(selected);}
            state.macros.status="Library ready. Macro playback integration is still in progress.".into();state.macros.failed=false;
        }
        Err(message)=>{state.macros.pending_selection=None;state.macros.status=message;state.macros.failed=true;}
    }
    if active(state){state.rebuild_fields();InvalidateRect(state.hwnd,null(),0);}
}
fn local(state:&mut State,change:Change)->Result<(),String> {
    let draft=state.macros.draft.as_mut().ok_or("Create or select a macro first")?;draft.apply(change)?;
    state.macros.name=draft.entry.name.clone();state.macros.status="Local macro changes · save to keep them".into();state.macros.failed=false;Ok(())
}
fn commit_fields(state:&mut State)->Result<(),String> {
    let Some(d)=&state.macros.draft else{return Ok(());};let id=d.entry.guid.clone();let old_name=d.entry.name.clone();
    if state.macros.name!=old_name {local(state,Change::Rename{guid:id.clone(),name:state.macros.name.clone()})?;}
    if let Some(index)=state.macros.event {
        let current=state.macros.draft.as_ref().and_then(|d|fixed_delay(&d.entry,index));
        if let Some(current)=current {let entered=state.macros.delay.trim().parse::<u32>().map_err(|_|"Enter a delay from0 to3600000milliseconds")?;if entered!=current{local(state,Change::SetDelay{guid:id,index,milliseconds:entered})?;}}
    }
    Ok(())
}
pub(super) unsafe fn action(state:&mut State,control:Control) {
    if matches!(control,Control::Open){state.macros.used=true;state.action(Action::Device(ID.into()));return;}
    if !active(state)||state.macros.busy{return;}
    let mutable=!state.read_only_smoke&&!state.restricted_actions;
    if !mutable&&!matches!(control,Control::List|Control::Select(_)|Control::Event(_)){return;}
    let result=(||->Result<(),String>{
        match control {
            Control::Open=>{},Control::List=>state.macros.picker=!state.macros.picker,
            Control::Select(id)=>{if state.macros.dirty(){return Err("Save or discard before switching macros".into());}state.macros.select(Some(id));},
            Control::Event(index)=>{if mutable{commit_fields(state)?;}state.macros.event=Some(index);state.macros.delay=state.macros.draft.as_ref().and_then(|d|fixed_delay(&d.entry,index)).map(|n|n.to_string()).unwrap_or_default();},
            Control::New=>{
                if state.macros.dirty(){return Err("Save or discard the current draft first".into());}
                let names=state.macros.data.as_ref().map(|d|d.library.entries.iter().map(|e|e.name.as_str()).collect::<Vec<_>>()).unwrap_or_default();
                let title=(1..=257).map(|n|format!("Macro {n}")).find(|n|!names.contains(&n.as_str())).ok_or("No available macro name")?;let id=identity()?;state.macros.pending_selection=Some(id.clone());request(state,Request::Change(Change::Create{guid:id,name:title}));
            }
            Control::Duplicate=>{if state.macros.dirty(){return Err("Save or discard the current draft first".into());}let d=state.macros.draft.as_ref().ok_or("Select a macro")?;let original=d.entry.guid.clone();let title=format!("{} Copy",d.entry.name);let id=identity()?;state.macros.pending_selection=Some(id.clone());request(state,Request::Change(Change::Duplicate{guid:original,new_guid:id,name:title}));},
            Control::Delete=>{if state.macros.dirty(){return Err("Save or discard the current draft first".into());}let id=state.macros.selected.clone().ok_or("Select a macro")?;request(state,Request::Change(Change::Delete(id)));},
            Control::Import=>{if state.macros.dirty(){return Err("Save or discard the current draft first".into());}PostMessageW(state.hwnd,BROWSE,0,0);},
            Control::InputMenu=>{PostMessageW(state.hwnd,INPUT_MENU,0,0);},
            Control::GestureMenu=>{PostMessageW(state.hwnd,INPUT_MENU,1,0);},
            Control::ChooseInput(input)=>{state.macros.input=Some(input);state.macros.gesture=Some(Gesture::Tap);},
            Control::ChooseGesture(gesture)=>{state.macros.gesture=Some(gesture);},
            Control::AddInput=>{
                commit_fields(state)?;let d=state.macros.draft.as_ref().ok_or("Select a macro")?;
                let index=state.macros.event.map_or(d.entry.events.len(),|i|(i+1).min(d.entry.events.len()));let guid=d.entry.guid.clone();
                let input=state.macros.input.clone().unwrap_or(Input::Keyboard("KEY_A".into()));let gesture=state.macros.gesture.unwrap_or(Gesture::Tap);
                let count=input_events::rows(&d.entry.events,index,&input,gesture).map_err(|e|e.to_string())?.len();
                local(state,Change::InsertInput{guid,index,input,gesture})?;state.macros.event=Some(index+count-1);state.macros.delay="1000".into();
            },
            Control::Save=>{commit_fields(state)?;let d=state.macros.draft.as_ref().ok_or("Select a macro")?;if d.dirty(){request(state,Request::Change(Change::Replace{guid:d.entry.guid.clone(),expected_revision:d.base_revision.clone(),record:d.entry.encoded_record.clone()}));}},
            Control::Discard=>{state.macros.select(state.macros.selected.clone());state.macros.status="Unsaved macro changes discarded".into();state.macros.failed=false;},
            Control::Undo|Control::Redo=>{let redo=matches!(control,Control::Redo);let d=state.macros.draft.as_mut().ok_or("Select a macro")?;d.history(redo)?;state.macros.name=d.entry.name.clone();state.macros.delay=state.macros.event.and_then(|i|fixed_delay(&d.entry,i)).map(|n|n.to_string()).unwrap_or("1000".into());},
            Control::AddDelay=>{commit_fields(state)?;let d=state.macros.draft.as_ref().ok_or("Select a macro")?;let index=state.macros.event.map_or(d.entry.events.len(),|i|(i+1).min(d.entry.events.len()));let guid=d.entry.guid.clone();let ms=state.macros.delay.trim().parse::<u32>().map_err(|_|"Enter a delay in milliseconds")?;local(state,Change::InsertDelay{guid,index,milliseconds:ms})?;state.macros.event=Some(index);},
            Control::ApplyDelay=>commit_fields(state)?,
            Control::DeleteEvent=>{let d=state.macros.draft.as_ref().ok_or("Select a macro")?;let index=state.macros.event.ok_or("Select an event")?;local(state,Change::DeleteEvent{guid:d.entry.guid.clone(),index})?;state.macros.event=None;},
            Control::Move(direction)=>{commit_fields(state)?;let d=state.macros.draft.as_ref().ok_or("Select a macro")?;let index=state.macros.event.ok_or("Select an event")?;local(state,Change::MoveEvent{guid:d.entry.guid.clone(),index,direction})?;state.macros.event=Some(index.checked_add_signed(direction as isize).ok_or("Invalid event move")?);},
        }Ok(())
    })();
    if let Err(message)=result{state.macros.status=message;state.macros.failed=true;}
    state.rebuild_fields();InvalidateRect(state.hwnd,null(),0);
}
pub(super) unsafe fn fields(state:&mut State) {
    if state.macros.draft.is_none(){return;}
    let left=((state.width as f32/state.scale)as i32-900).max(10)/2;
    for (id,text,x,y,w) in [(NAME,state.macros.name.clone(),left+10,196,180),(DELAY,state.macros.delay.clone(),left+10,310,180)] {
        state.retained_fields.remove(&id);state.field(id,&text,x,y,w);
        if let Some(hwnd)=state.fields.get(&id){SendMessageW(*hwnd,EM_SETLIMITTEXT,if id==NAME{128}else{7},0);SendMessageW(*hwnd,EM_SETREADONLY,(state.read_only_smoke||state.restricted_actions||state.macros.busy) as usize,0);}
    }
}
pub(super) fn command(state:&mut State,id:u16,notification:u16)->bool {
    if !active(state)||notification!=EN_CHANGE as u16||!matches!(id,NAME|DELAY){return false;}
    if state.read_only_smoke||state.restricted_actions||state.macros.busy{return true;}
    let text=state.raw_text(id);if id==NAME{state.macros.name=text;}else{state.macros.delay=text;}unsafe{InvalidateRect(state.hwnd,null(),0);}true
}
unsafe fn button(p:&mut Painter,text:&str,x:i32,y:i32,width:i32,control:Control,enabled:bool) {
    if enabled{p.button(text,x,y,width,Action::MacroLibrary(control),false);}else{p.fill(x,y,width,28,0x242424);unsafe{p.text(text,x,y,width,28,p.fonts.normal,MUTED,DT_CENTER);}}
}
fn event_label(value:&Value)->String {
    if let Some(label)=input_events::label(value){return label;}
    match value["Type"].as_u64(){Some(0)if value["Number"].is_object()=>"Random delay".into(),Some(0)=>format!("Delay · {} seconds",value["Number"].as_str().map(|s|s.chars().take(80).collect::<String>()).unwrap_or_else(||value["Number"].to_string())),Some(1)=>"Keyboard function".into(),Some(2)=>"Mouse function".into(),Some(7)=>"Nested macro".into(),Some(6)=>"Loop".into(),_=>"Retained source event".into()}
}
pub(super) unsafe fn paint(state:&State,p:&mut Painter,width:i32,height:i32) {
    let view=&state.macros;let left=(width-900).max(10)/2;let timeline=left+210;let can=!view.busy&&!state.read_only_smoke&&!state.restricted_actions;let draft=view.draft.as_ref();
    p.fill(0,80,width,height-80,BG);p.line(0,127,width,0);
    let title=draft.map_or("Select a macro",|d|d.entry.name.as_str());p.text(&format!("{}{}",title,if view.dirty(){" *"}else{""}),46,87,224,28,p.fonts.normal,TEXT,DT_LEFT);
    p.hits.push(Hit{rect:p.rect(46,87,230,28),action:Action::MacroLibrary(Control::List)});
    button(p,"NEW MACRO",300,87,120,Control::New,can);button(p,"IMPORT",430,87,100,Control::Import,can);
    p.fill(left,148,200,(height-210).max(100),PANEL);p.fill(timeline,148,690,(height-210).max(100),PANEL);
    p.text("MY MACROS",width/2-55,87,110,28,p.fonts.normal,GREEN,DT_CENTER);
    p.text("NAME",left+10,162,180,28,p.fonts.small,TEXT,DT_LEFT);
    button(p,"DUPLICATE",left+10,240,85,Control::Duplicate,can&&draft.is_some());button(p,"DELETE",left+105,240,85,Control::Delete,can&&draft.is_some_and(|d|!d.entry.referenced));
    p.text("DELAY · MILLISECONDS",left+10,276,180,28,p.fonts.small,TEXT,DT_LEFT);
    button(p,"ADD DELAY",left+10,354,180,Control::AddDelay,can&&draft.is_some());button(p,"APPLY DELAY",left+10,390,180,Control::ApplyDelay,can&&draft.is_some_and(|d|view.event.and_then(|i|fixed_delay(&d.entry,i)).is_some()));
    button(p,"MOVE UP",left+10,432,85,Control::Move(-1),can&&view.event.is_some());button(p,"DOWN",left+105,432,85,Control::Move(1),can&&view.event.is_some());button(p,"DELETE EVENT",left+10,470,180,Control::DeleteEvent,can&&view.event.is_some());
    let input=match &view.input{Some(Input::Mouse(button))=>input_events::MOUSE_NAMES.get(*button as usize).copied().unwrap_or("Choose input"),Some(Input::Keyboard(id))=>input_events::keys().iter().find(|key|&key.id==id).map_or("Choose input",|key|key.name.as_str()),None=>"A"};
    button(p,&format!("INPUT: {input}"),left+10,514,180,Control::InputMenu,can&&draft.is_some());
    let complete_action=matches!(view.input,Some(Input::Mouse(5..=9)));
    let gesture=if complete_action{"Perform action"}else{match view.gesture.unwrap_or(Gesture::Tap){Gesture::Tap=>"Press and release",Gesture::Down=>"Down",Gesture::Up=>"Up"}};
    button(p,gesture,left+10,550,180,Control::GestureMenu,can&&draft.is_some()&&!complete_action);button(p,"ADD INPUT",left+10,586,180,Control::AddInput,can&&draft.is_some());
    button(p,"UNDO",timeline+10,160,80,Control::Undo,can&&draft.is_some_and(|d|!d.undo.is_empty()));button(p,"REDO",timeline+100,160,80,Control::Redo,can&&draft.is_some_and(|d|!d.redo.is_empty()));
    button(p,"DISCARD",timeline+470,160,100,Control::Discard,can&&view.dirty());button(p,"SAVE",timeline+580,160,100,Control::Save,can&&view.dirty());
    if let Some(d)=draft {
        let visible:Vec<_>=d.entry.events.iter().enumerate().filter(|(_,e)|e["Type"]!="actionBar").collect();
        for (offset,(index,value)) in visible.into_iter().skip(view.scroll).take(((height-285).max(0)/38) as usize).enumerate(){let y=204+offset as i32*38;let chosen=view.event==Some(index);p.fill(timeline+10,y,670,36,if chosen{0x343434}else{0x151515});p.text(&event_label(value),timeline+20,y,650,36,p.fonts.normal,if chosen{GREEN}else{TEXT},DT_LEFT);p.hits.push(Hit{rect:p.rect(timeline+10,y,670,36),action:Action::MacroLibrary(Control::Event(index))});}
    } else {p.text("Create a macro or import definitions from a Synapse4 profile export.",timeline+20,220,650,70,p.fonts.normal,MUTED,DT_LEFT|DT_WORDBREAK);}
    let status=if view.status.is_empty(){"Macro playback integration is still in progress."}else{&view.status};p.text(status,30,height-50,width-60,40,p.fonts.normal,if view.failed{0xfd4949}else{MUTED},DT_LEFT|DT_WORDBREAK);
    if let Some(data)=&view.data {if !data.library.missing.is_empty(){p.text(&format!("{} linked macro definition(s) missing · import an export containing their macros",data.library.missing.len()),30,height-80,width-60,25,p.fonts.small,MUTED,DT_LEFT);}}
    if view.picker {if let Some(data)=&view.data {let count=data.library.entries.len().saturating_sub(view.picker_scroll).min(8);p.fill(46,116,290,count as i32*28,0);for (offset,entry) in data.library.entries.iter().skip(view.picker_scroll).take(8).enumerate(){let y=116+offset as i32*28;p.text(&entry.name,52,y,278,28,p.fonts.normal,TEXT,DT_LEFT);p.hits.push(Hit{rect:p.rect(46,y,290,28),action:Action::MacroLibrary(Control::Select(entry.guid.clone()))});}}}
}
struct InputMenu(HMENU);
impl Drop for InputMenu {fn drop(&mut self){unsafe{DestroyMenu(self.0);}}}
unsafe fn menu()->Result<InputMenu,String>{let handle=CreatePopupMenu();if handle.is_null(){Err("Could not open input choices".into())}else{Ok(InputMenu(handle))}}
unsafe fn append(menu:&InputMenu,id:usize,title:&str)->Result<(),String>{if AppendMenuW(menu.0,MF_STRING,id,wide(&title.replace('&',"&&")).as_ptr())==0{Err("Could not populate input choices".into())}else{Ok(())}}
pub(super) unsafe fn choose_input(hwnd:HWND,gesture:bool)->Result<Option<Control>,String>{
    let root=menu()?;let mut choices=vec![];
    if gesture {for (label,value) in [("Press and release",Gesture::Tap),("Down",Gesture::Down),("Up",Gesture::Up)] {choices.push(Control::ChooseGesture(value));append(&root,choices.len(),label)?;}}
    else {
        let mut groups:BTreeMap<&str,Vec<&input_events::Key>>=BTreeMap::new();for key in input_events::keys(){groups.entry(if key.category.is_empty(){"Other keys"}else{&key.category}).or_default().push(key);}
        for (category,keys) in groups {let child=menu()?;for key in keys {choices.push(Control::ChooseInput(Input::Keyboard(key.id.clone())));append(&child,choices.len(),&key.name)?;}
            if AppendMenuW(root.0,MF_POPUP,child.0 as usize,wide(category).as_ptr())==0{return Err("Could not populate keyboard choices".into());}std::mem::forget(child);
        }
        let mouse=menu()?;for (button,name) in input_events::MOUSE_NAMES.iter().enumerate(){choices.push(Control::ChooseInput(Input::Mouse(button as u8)));append(&mouse,choices.len(),name)?;}
        if AppendMenuW(root.0,MF_POPUP,mouse.0 as usize,wide("Mouse functions").as_ptr())==0{return Err("Could not populate mouse choices".into());}std::mem::forget(mouse);
    }
    let mut point:POINT=std::mem::zeroed();if GetCursorPos(&mut point)==0{return Err("Could not position input choices".into());}
    let result=TrackPopupMenu(root.0,TPM_RETURNCMD|TPM_NONOTIFY,point.x,point.y,0,hwnd,null()) as usize;
    Ok(result.checked_sub(1).and_then(|index|choices.get(index)).cloned())
}
pub(super) unsafe fn wheel(state:&mut State,delta:i16) {
    let count=state.macros.draft.as_ref().map_or(0,|d|d.entry.events.len());
    let view=&mut state.macros;if view.picker{let max=view.data.as_ref().map_or(0,|d|d.library.entries.len().saturating_sub(8));view.picker_scroll=if delta<0{view.picker_scroll.saturating_add(1).min(max)}else{view.picker_scroll.saturating_sub(1)};}else{view.scroll=if delta<0{view.scroll.saturating_add(3).min(count.saturating_sub(1))}else{view.scroll.saturating_sub(3)};}InvalidateRect(state.hwnd,null(),0);
}
pub(super) unsafe fn browse(hwnd:HWND)->Result<Option<PathBuf>,String> {
    let mut file=vec![0u16;32768];let filter=wide("Synapse4 profile exports\0*.synapse4\0JSON files\0*.json\0\0");let title=wide("Import macro definitions from a profile export");let mut options:OPENFILENAMEW=std::mem::zeroed();options.lStructSize=std::mem::size_of::<OPENFILENAMEW>()as u32;options.hwndOwner=hwnd;options.lpstrFile=file.as_mut_ptr();options.nMaxFile=file.len()as u32;options.lpstrFilter=filter.as_ptr();options.lpstrTitle=title.as_ptr();options.Flags=OFN_FILEMUSTEXIST|OFN_PATHMUSTEXIST|OFN_NOCHANGEDIR;
    if GetOpenFileNameW(&mut options)==0{let error=CommDlgExtendedError();return if error==0{Ok(None)}else{Err(format!("Import picker failed ({error})"))};}
    let end=file.iter().position(|v|*v==0).ok_or("Import path is not terminated")?;Ok(Some(PathBuf::from(String::from_utf16(&file[..end]).map_err(|e|e.to_string())?)))
}
#[cfg(test)] pub(super) fn diagnostics(state:&State)->Value {
    let v=&state.macros;json!({"open":active(state),"busy":v.busy,"menu_open":v.popup,"dirty":v.dirty(),"selected":v.selected,"name":v.name,"delay":v.delay,"event":v.event,"status":v.status,"failed":v.failed,"entries":v.data.as_ref().map(|d|d.library.entries.iter().map(|e|json!({"guid":e.guid,"name":e.name,"referenced":e.referenced})).collect::<Vec<_>>()).unwrap_or_default(),"events":v.draft.as_ref().map(|d|&d.entry.events),"undo":v.draft.as_ref().map_or(0,|d|d.undo.len()),"redo":v.draft.as_ref().map_or(0,|d|d.redo.len())})
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn local_history_is_bounded_and_keeps_reference_context() {
        let store=library::edit(&Store::default(),Change::Create{guid:"history".into(),name:"Original".into()}).unwrap();
        let mut entry=library::read(&store).unwrap().entries.remove(0);entry.referenced=true;let mut draft=Draft::new(entry);
        for n in 0..40 {draft.apply(Change::Rename{guid:"history".into(),name:format!("Edit {n}")}).unwrap();}
        assert_eq!(draft.undo.len(),20);assert!(draft.dirty());draft.history(false).unwrap();assert_eq!(draft.entry.name,"Edit 38");assert!(draft.entry.referenced);draft.history(true).unwrap();assert_eq!(draft.entry.name,"Edit 39");assert!(draft.entry.referenced);
        draft.history(false).unwrap();draft.apply(Change::Rename{guid:"history".into(),name:"Branch".into()}).unwrap();assert!(draft.redo.is_empty());assert!(draft.undo.iter().chain(&draft.redo).map(String::len).sum::<usize>()<=4*1024*1024);
    }
}
