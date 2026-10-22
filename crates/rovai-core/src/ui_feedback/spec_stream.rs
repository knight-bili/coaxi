//! 固定 @json-render/core 0.21 的补丁语义。节点编号保留 copy 的对象引用关系。
//! 仅用于核对卡片字段；不执行脚本、表达式、动作或文件访问。
use super::*;

type Id=usize;
#[derive(Clone)]enum Node{Undefined,Scalar(Value),Array(Vec<Id>),Object(BTreeMap<String,Id>)}
struct Graph{nodes:Vec<Node>}
impl Graph{
    fn push(&mut self,node:Node)->Result<Id>{ensure!(self.nodes.len()<65_536,"feedback.card_spec_limit");let id=self.nodes.len();self.nodes.push(node);Ok(id)}
    fn import(&mut self,value:&Value,depth:usize)->Result<Id>{
        ensure!(depth<128,"feedback.card_spec_limit");let node=match value{
            Value::Array(rows)=>Node::Array(rows.iter().map(|row|self.import(row,depth+1)).collect::<Result<Vec<_>>>()?),
            Value::Object(rows)=>{let mut result=BTreeMap::new();for(key,value)in rows{result.insert(key.clone(),self.import(value,depth+1)?);}Node::Object(result)},
            _=>Node::Scalar(value.clone())
        };self.push(node)
    }
    fn index(part:&str)->Result<usize>{
        // SDK 对数组使用 parseInt；只准入有界的非负数组下标，不能分配任意稀疏数组。
        let part=part.trim_start();let part=part.strip_prefix('+').unwrap_or(part);let digits=part.chars().take_while(char::is_ascii_digit).collect::<String>();
        let index=digits.parse::<usize>().context("feedback.patch_path")?;ensure!(index<4096,"feedback.card_spec_limit");Ok(index)
    }
    fn child(&self,id:Id,part:&str)->Option<Id>{match &self.nodes[id]{Node::Object(rows)=>rows.get(part).copied(),Node::Array(rows)=>Self::index(part).ok().and_then(|index|rows.get(index).copied()),_=>None}}
    fn read(&self,root:Id,path:&str)->Result<Option<Id>>{if path.is_empty()||path=="/"{return Ok(Some(root))}let mut current=Some(root);for part in pointer(path)?{current=current.and_then(|id|self.child(id,&part));}Ok(current)}
    fn object(&self,id:Id)->bool{matches!(&self.nodes[id],Node::Array(_)|Node::Object(_)|Node::Scalar(Value::Null))}
    fn assign(&mut self,parent:Id,key:&str,item:Id,insert:bool)->Result<()>{
        if matches!(&self.nodes[parent],Node::Array(_)){
            let len=match &self.nodes[parent]{Node::Array(rows)=>rows.len(),_=>unreachable!()};let index=if key=="-"{len}else{Self::index(key)?};
            if insert{if let Node::Array(rows)=&mut self.nodes[parent]{rows.insert(index.min(len),item)}}
            else{let mut holes=Vec::new();for _ in len..index{holes.push(self.push(Node::Undefined)?);}if let Node::Array(rows)=&mut self.nodes[parent]{rows.extend(holes);if index<rows.len(){rows[index]=item}else{rows.push(item)}}}
        }else if let Node::Object(rows)=&mut self.nodes[parent]{rows.insert(key.into(),item);}else{bail!("feedback.patch_path")}
        Ok(())
    }
    fn write(&mut self,root:Id,path:&str,item:Id,insert:bool)->Result<()>{
        let parts=pointer(path)?;let(key,parents)=parts.split_last().context("feedback.patch_path")?;let mut current=root;
        for(index,part)in parents.iter().enumerate(){
            let child=self.child(current,part);let next=if let Some(child)=child.filter(|id|self.object(*id)){child}else{
                let next=&parts[index+1];let numeric=next=="-"||!next.is_empty()&&next.bytes().all(|b|b.is_ascii_digit());let id=self.push(if numeric{Node::Array(Vec::new())}else{Node::Object(BTreeMap::new())})?;self.assign(current,part,id,false)?;id
            };current=next;
        }
        self.assign(current,key,item,insert)
    }
    fn remove(&mut self,root:Id,path:&str)->Result<()>{
        let parts=pointer(path)?;let(key,parents)=parts.split_last().context("feedback.patch_path")?;let mut current=root;
        for part in parents{let Some(child)=self.child(current,part).filter(|id|self.object(*id))else{return Ok(())};current=child;}
        match &mut self.nodes[current]{Node::Object(rows)=>{rows.remove(key);},Node::Array(rows)=>{if let Ok(index)=Self::index(key){if index<rows.len(){rows.remove(index);}}},_=>bail!("feedback.patch_path")};Ok(())
    }
    fn value(&self,id:Id,active:&mut BTreeSet<Id>,count:&mut usize)->Result<Value>{
        *count+=1;ensure!(*count<=131_072&&active.len()<128&&active.insert(id),"feedback.card_cycle");
        let value=match &self.nodes[id]{Node::Undefined=>Value::Null,Node::Scalar(value)=>value.clone(),Node::Array(rows)=>Value::Array(rows.iter().map(|id|self.value(*id,active,count)).collect::<Result<Vec<_>>>()?),Node::Object(rows)=>{let mut map=serde_json::Map::new();for(key,id)in rows{if !matches!(self.nodes[*id],Node::Undefined){map.insert(key.clone(),self.value(*id,active,count)?);}}Value::Object(map)}};
        active.remove(&id);Ok(value)
    }
    fn export(&self,id:Id)->Result<Value>{self.value(id,&mut BTreeSet::new(),&mut 0)}
    fn equivalent(&self,left:Option<Id>,right:Id,active:&mut BTreeSet<(Id,Id)>)->Result<bool>{
        let Some(left)=left else{return Ok(matches!(self.nodes[right],Node::Undefined))};if left==right{return Ok(true)}
        ensure!(active.len()<128&&active.insert((left,right)),"feedback.card_cycle");
        let same=match(&self.nodes[left],&self.nodes[right]){
            (Node::Undefined,Node::Undefined)=>true,(Node::Scalar(a),Node::Scalar(b))=>a==b,
            (Node::Array(a),Node::Array(b))=>{if a.len()!=b.len(){false}else{let mut same=true;for(a,b)in a.iter().zip(b){same&=self.equivalent(Some(*a),*b,active)?;}same}},
            (Node::Object(a),Node::Object(b))=>{if a.len()!=b.len(){false}else{let mut same=true;for(key,a)in a{let Some(b)=b.get(key)else{same=false;break};same&=self.equivalent(Some(*a),*b,active)?;}same}},_=>false
        };active.remove(&(left,right));Ok(same)
    }
}
fn pointer(path:&str)->Result<Vec<String>>{
    ensure!(path.len()<=512,"feedback.invalid_pointer");let path=path.strip_prefix('/').unwrap_or(path);
    path.split('/').map(|part|{let part=part.replace("~1","/").replace("~0","~");ensure!(!matches!(part.as_str(),"__proto__"|"constructor"|"prototype"),"feedback.field_key_forbidden");Ok(part)}).collect()
}
fn patch(line:&str)->Option<Value>{let line=line.trim();if !line.starts_with('{'){return None}let value:Value=serde_json::from_str(line).ok()?;if value["op"].as_str().is_some_and(|op|!op.is_empty())&&value.get("path").is_some(){Some(value)}else{None}}
pub fn compile(source:&str)->Result<Value>{
    ensure!(source.lines().find(|line|!line.trim().is_empty()).and_then(patch).is_some(),"feedback.card_spec_unavailable");
    let mut graph=Graph{nodes:Vec::new()};let root=graph.import(&json!({"root":"","elements":{}}),0)?;let mut count=0;
    for line in source.lines(){let Some(patch)=patch(line)else{continue};let path=patch["path"].as_str().context("feedback.patch_path")?;
        if path.contains("__proto__")||patch["from"].as_str().is_some_and(|from|from.contains("__proto__")){continue}
        count+=1;ensure!(count<=4096,"feedback.card_patch_limit");
        match patch["op"].as_str(){
            Some("add"|"replace")=>{let item=if let Some(value)=patch.get("value"){graph.import(value,0)?}else{graph.push(Node::Undefined)?};graph.write(root,path,item,patch["op"]=="add")?;},
            Some("remove")=>graph.remove(root,path)?,
            Some("move"|"copy")=>{if let Some(from)=patch["from"].as_str().filter(|from|!from.is_empty()){let item=if let Some(item)=graph.read(root,from)?{item}else{graph.push(Node::Undefined)?};if patch["op"]=="move"{graph.remove(root,from)?;}graph.write(root,path,item,true)?;}},
            Some("test")=>{let expected=if let Some(value)=patch.get("value"){graph.import(value,0)?}else{graph.push(Node::Undefined)?};ensure!(graph.equivalent(graph.read(root,path)?,expected,&mut BTreeSet::new())?,"feedback.patch_test");},
            _=>{}
        }
    }
    let value=graph.export(root)?;ensure!(serde_json::to_vec(&value)?.len()<=1024*1024&&value["root"].is_string()&&value["elements"].is_object(),"feedback.card_spec_limit");Ok(value)
}
