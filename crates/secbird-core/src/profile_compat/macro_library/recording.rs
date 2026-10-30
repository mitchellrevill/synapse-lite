//! Explicit, caller-fed focused recording. No hooks, clocks, threads or input output.
use super::{input_events::{Input,Gesture},Change};
#[derive(Default)]
pub struct Recording { events:Vec<(u64,Input,Gesture)>, held:Vec<(Input,usize)>, last:u64 }
impl Recording {
    /// At most 240 transitions and 60 seconds. Repeats and orphan releases are ignored.
    pub fn capture(&mut self,ms:u64,input:Input,gesture:Gesture)->Result<(),String> {
        if ms<self.last || ms>60_000 || self.events.len()>=240 {return Err("Recording limit reached (60 seconds / 240 transitions)".into());}
        if gesture==Gesture::Tap {return Err("Recording requires explicit transitions".into());}
        super::input_events::rows(&[],0,&input,gesture).map_err(|e|e.to_string())?;
        let held=self.held.iter().position(|(i,_)|i==&input);
        match (gesture,held) {
            (Gesture::Down,None)=>self.held.push((input.clone(),self.events.len())),
            (Gesture::Up,Some(i))=>{self.held.remove(i);},
            _=>return Ok(()),
        }
        self.events.push((ms,input,gesture));self.last=ms;Ok(())
    }
    /// Incomplete downs are omitted, never replaced by invented releases.
    pub fn changes(self,guid:&str,index:usize)->Vec<Change> {
        let mut changes=vec![];let mut cursor=index;let mut previous=None;
        for (i,(ms,input,gesture)) in self.events.into_iter().enumerate() {
            if self.held.iter().any(|(_,down)|*down==i){continue;}
            if let Some(last)=previous {changes.push(Change::InsertDelay{guid:guid.into(),index:cursor,milliseconds:(ms-last) as u32});cursor+=1;}
            changes.push(Change::InsertInput{guid:guid.into(),index:cursor,input,gesture});cursor+=1;previous=Some(ms);
        }
        changes
    }
}
#[cfg(test)] mod tests {
 use super::*;
 fn a()->Input{Input::Keyboard("KEY_A".into())}
 #[test] fn pairs_repeats_orphans_and_delays(){let mut r=Recording::default();r.capture(0,a(),Gesture::Up).unwrap();r.capture(5,a(),Gesture::Down).unwrap();r.capture(10,a(),Gesture::Down).unwrap();r.capture(25,a(),Gesture::Up).unwrap();let c=r.changes("a",1);assert_eq!(c.len(),3);assert!(matches!(c[1],Change::InsertDelay{milliseconds:20,..}));}
 #[test] fn focus_loss_drops_unfinished_down(){let mut r=Recording::default();r.capture(1,a(),Gesture::Down).unwrap();assert!(r.changes("a",1).is_empty());}
 #[test] fn bounds_and_clock(){let mut r=Recording::default();r.capture(10,a(),Gesture::Down).unwrap();assert!(r.capture(9,a(),Gesture::Up).is_err());assert!(r.capture(60001,a(),Gesture::Up).is_err());for i in 1..240 {r.capture(10+i,a(),if i%2==1{Gesture::Up}else{Gesture::Down}).unwrap();}assert!(r.capture(1000,a(),Gesture::Down).is_err());}
 #[test] fn source_rows_use_existing_authoring_contract(){use super::super::*;let s=edit(&crate::Store::default(),Change::Create{guid:"r".into(),name:"Record".into()}).unwrap();let mut e=read(&s).unwrap().entries.remove(0);let mut r=Recording::default();r.capture(0,Input::Mouse(1),Gesture::Down).unwrap();r.capture(30,Input::Mouse(1),Gesture::Up).unwrap();for c in r.changes("r",1){e=describe(&edit_definition(&e.encoded_record,c).unwrap()).unwrap();}assert_eq!(e.events[1]["Id"],e.events[3]["Id"]);assert_eq!(e.events[1]["MouseEvent"]["MouseButton"],1);assert_eq!(e.events[2]["Number"],"0.03");}
}
