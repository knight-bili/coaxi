//! Read-only filesystem port. The helper gets exactly one selected snapshot
//! root, with process-wide byte limits shared by parser helper threads.
use std::{io::{self,Read,Seek,SeekFrom,Write},path::{Path,PathBuf},sync::{OnceLock,atomic::{AtomicU64,Ordering}}};
pub use std::fs::{Metadata,ReadDir};
const MAX_FILE:u64=128*1024*1024;
const MAX_READ:u64=512*1024*1024;
struct Scope{root:PathBuf,read:AtomicU64}
static SCOPE:OnceLock<Scope>=OnceLock::new();
fn denied()->io::Error{io::Error::new(io::ErrorKind::PermissionDenied,"history.snapshot_read_scope_required")}
pub fn initialize(path:&Path)->io::Result<()>{
    if !path.is_absolute(){return Err(denied())}let root=std::fs::canonicalize(path)?;if !root.is_dir(){return Err(denied())}
    reject_links(&root)?;SCOPE.set(Scope{root,read:AtomicU64::new(0)}).map_err(|_|denied())
}
pub fn root()->PathBuf{SCOPE.get().map(|scope|scope.root.clone()).unwrap_or_default()}
fn reject_links(path:&Path)->io::Result<()>{let mut current=PathBuf::new();for part in path.components(){current.push(part);if matches!(part,std::path::Component::Prefix(_)){continue}let value=std::fs::symlink_metadata(&current)?;if value.file_type().is_symlink(){return Err(denied())}#[cfg(windows)]{use std::os::windows::fs::MetadataExt;if value.file_attributes()&0x400!=0{return Err(denied())}}}Ok(())}
fn checked(path:&Path)->io::Result<PathBuf>{
    let scope=SCOPE.get().ok_or_else(denied)?;let path=if path.is_absolute(){path.to_owned()}else{scope.root.join(path)};
    if !path.starts_with(&scope.root)||path.components().any(|component|matches!(component,std::path::Component::ParentDir)){return Err(denied())}reject_links(&path)?;let resolved=std::fs::canonicalize(path)?;if !resolved.starts_with(&scope.root){return Err(denied())}Ok(resolved)
}
fn account(bytes:usize)->io::Result<()>{let scope=SCOPE.get().ok_or_else(denied)?;if scope.read.fetch_add(bytes as u64,Ordering::Relaxed).saturating_add(bytes as u64)>MAX_READ{return Err(io::Error::new(io::ErrorKind::InvalidData,"history.snapshot_read_budget_exceeded"))}Ok(())}
pub fn metadata(path:impl AsRef<Path>)->io::Result<Metadata>{std::fs::metadata(checked(path.as_ref())?)}
pub fn read_dir(path:impl AsRef<Path>)->io::Result<ReadDir>{std::fs::read_dir(checked(path.as_ref())?)}
pub fn read(path:impl AsRef<Path>)->io::Result<Vec<u8>>{let mut file=File::open(path)?;let mut value=Vec::new();(&mut file).take(MAX_FILE+1).read_to_end(&mut value)?;if value.len()as u64>MAX_FILE{return Err(denied())}Ok(value)}
pub fn read_to_string(path:impl AsRef<Path>)->io::Result<String>{String::from_utf8(read(path)?).map_err(|error|io::Error::new(io::ErrorKind::InvalidData,error))}
pub fn create_dir_all(_path:impl AsRef<Path>)->io::Result<()>{Err(denied())}
#[derive(Debug)]pub struct File{file:std::fs::File}
impl File{
    pub fn open(path:impl AsRef<Path>)->io::Result<Self>{let path=checked(path.as_ref())?;let file=std::fs::File::open(path)?;if !file.metadata()?.is_file()||file.metadata()?.len()>MAX_FILE{return Err(denied())}Ok(Self{file})}
    pub fn metadata(&self)->io::Result<Metadata>{self.file.metadata()}
    pub fn sync_all(&self)->io::Result<()>{Err(denied())}
}
impl Read for File{fn read(&mut self,bytes:&mut[u8])->io::Result<usize>{let count=self.file.read(bytes)?;account(count)?;Ok(count)}}
impl Seek for File{fn seek(&mut self,value:SeekFrom)->io::Result<u64>{self.file.seek(value)}}
impl Write for File{fn write(&mut self,_bytes:&[u8])->io::Result<usize>{Err(denied())}fn flush(&mut self)->io::Result<()>{Err(denied())}}
#[derive(Default)]pub struct OpenOptions{mutating:bool}
impl OpenOptions{
    pub fn new()->Self{Self::default()}
    pub fn read(&mut self,_value:bool)->&mut Self{self}
    pub fn write(&mut self,value:bool)->&mut Self{self.mutating|=value;self}
    pub fn append(&mut self,value:bool)->&mut Self{self.mutating|=value;self}
    pub fn create(&mut self,value:bool)->&mut Self{self.mutating|=value;self}
    pub fn truncate(&mut self,value:bool)->&mut Self{self.mutating|=value;self}
    pub fn open(&self,path:impl AsRef<Path>)->io::Result<File>{if self.mutating{Err(denied())}else{File::open(path)}}
}
