//! Fixed Breathing gradient authoring over the recovered nine-stop converter.
use super::*;

pub(super) fn validate_layer(item:&Object)->Result<()> {
    if item.text("name")?!="breathing"||item.get("value").is_some_and(|v|v.get()!="5") {return Err(invalid("selected layer is not Studio Breathing5"));} Ok(())
}
fn authorable(params:&Object)->Result<()> {
    let random=params.get("randomColor").map(|v|serde_json::from_str::<bool>(v.get()).map_err(invalid)).transpose()?.unwrap_or(false);
    if random {return Err(invalid("random Breathing gradient editing is unavailable; settings were preserved"));} Ok(())
}
fn stops(params:&Object)->Result<Vec<GradientStop>> {
    authorable(params)?;
    if let Some(stops)=spectrum::gradient(params)? {return Ok(stops);}
    let color=|field:&str|->Result<Option<u32>>{params.get(field).map(|v|serde_json::from_str(v.get()).map_err(invalid)).transpose()};
    let second=color("color2")?;let first=color("color")?.or(second);
    Ok([(0,Some(0)),(12,Some(0)),(25,first),(38,first),(50,Some(0)),(62,Some(0)),(75,second),(88,second),(100,Some(0))].into_iter().map(|(position,color)|GradientStop{position,color}).collect())
}
fn merged(item:&Object,region:Option<&Object>)->Result<Object> {
    let mut params=Object(vec![]);
    if let Some(raw)=item.get("params") {for (name,value) in Object::parse(raw)?.0 {params.set(&name,Some(&value));}}
    if let Some(region)=region {for (name,value) in Object::parse(region.get("params").ok_or_else(||invalid("region has no params"))?)?.0 {params.set(&name,Some(&value));}}
    Ok(params)
}
fn validate(stops:&[GradientStop])->Result<()> {
    if stops.len()<2||stops.len()>10||stops.windows(2).any(|p|p[0].position>=p[1].position) {return Err(invalid("Breathing needs two to ten strictly ordered stops"));}
    for stop in stops {spectrum::validate_stop(stop)?;}
    // Endpoint expansion and native palette admission must succeed before commit.
    crate::breathing_lighting::BreathingCycle::studio(None,None,Some(stops),Some(6000))?;Ok(())
}
pub(super) fn apply(params:&mut Object,defaults:&Object,edit:StopEdit)->Result<()> {
    let mut combined=Object::parse(&encode(defaults)?)?;
    for (name,value) in &params.0 {combined.set(name,Some(value));}
    let effective=stops(&combined)?;validate(&effective)?;
    if params.get("colorStops").is_none(){
        if let Some(raw)=combined.get("colorStops") {params.set("colorStops",Some(raw));}
        else {params.set("colorStops",Some(&encode(&effective)?));}
    }
    spectrum::edit_stop(params,edit,10)?;
    validate(&stops(params)?)
}
impl Draft {
    fn breathing_gradient_params(&self,id:&str,selection:&[LedSelection])->Result<Vec<Object>> {
        let selection=selected(selection)?;let item=layer(self,id,None)?;validate_layer(&item)?;
        if selection.is_empty(){return Ok(vec![]);}inventory(self,&selection)?;
        let mut seen=BTreeSet::new();let mut result=vec![];
        for raw in entries(&item,"regions")? {
            let region=Object::parse(&raw)?;let mut matched=false;
            for raw in entries(&region,"devices")? {let device=Object::parse(&raw)?;let device_id=device.text("id")?;
                for raw in entries(&device,"leds")? {let key=led(&raw,&device_id)?;if selection.contains(&key) {
                    if !seen.insert(key){return Err(invalid("overlapping selected region memberships"));}matched=true;
                }}
            }
            if matched {result.push(merged(&item,Some(&region))?);}
        }
        if seen.len()<selection.len(){result.push(merged(&item,None)?);}Ok(result)
    }
    pub fn breathing_selection_gradient(&self,id:&str,selection:&[LedSelection])->Result<SelectedGradient> {
        let params=self.breathing_gradient_params(id,selection)?;
        let Some(first)=params.first() else{return Ok(SelectedGradient::Empty);};let first=stops(first)?;
        for p in &params[1..] {if stops(p)?!=first{return Ok(SelectedGradient::Mixed);}}
        Ok(SelectedGradient::Uniform(Some(first)))
    }
    pub fn edit_breathing_stop(&mut self,id:&str,selection:&[LedSelection],edit:StopEdit)->Result<bool> {
        match edit {StopEdit::Set{position,color,..}|StopEdit::Insert{position,color}=>spectrum::validate_stop(&GradientStop{position,color})?,StopEdit::Delete{..}=>{}}
        if !matches!(self.breathing_selection_gradient(id,selection)?,SelectedGradient::Uniform(Some(_))) {return Err(invalid("select one uniform Breathing gradient before editing"));}
        // Materialize inherited stops per selected membership without changing
        // duration, playback flags, untouched regions or unknown raw leaves.
        let item=layer(self,id,None)?;let defaults=merged(&item,None)?;
        self.edit_regions(id,selection,RegionEdit::BreathingGradient(edit,&defaults))
    }
}
