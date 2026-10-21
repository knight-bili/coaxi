//! Bounded owner-requested CLI operations share Core's process-tree ownership.
use anyhow::{Context,Result,ensure};
use crate::managed_process::{ManagedProcess,ManagedProcessLaunchSpec,ManagedProcessPurpose,ManagedStdinPolicy,ManagedWindowsArgvDialect};
use std::{sync::Arc,time::Duration};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
async fn drain(mut reader:impl tokio::io::AsyncRead+Unpin,log:Arc<tokio::sync::Mutex<Vec<u8>>>)->std::io::Result<()> {
    let mut buffer=[0u8;4096];loop{let count=reader.read(&mut buffer).await?;if count==0{return Ok(())}let mut log=log.lock().await;log.extend_from_slice(&buffer[..count]);if log.len()>64*1024{let remove=log.len()-64*1024;log.drain(..remove);}}
}
pub(crate) async fn run_bounded_command(mut command:tokio::process::Command,owner:&str,timeout:Duration,cancel:CancellationToken,log:Arc<tokio::sync::Mutex<Vec<u8>>>)->Result<()> {
    command.stdin(std::process::Stdio::null());
    let spec=ManagedProcessLaunchSpec::capture(&command,ManagedProcessPurpose::RuntimeOneShot,ManagedStdinPolicy::Null,ManagedWindowsArgvDialect::MicrosoftCrt,owner.to_owned())?;
    let mut child=ManagedProcess::spawn(spec)?;
    let out=tokio::spawn(drain(child.take_stdout().context("office.stdout_missing")?,log.clone()));
    let err=tokio::spawn(drain(child.take_stderr().context("office.stderr_missing")?,log));
    let waited=tokio::select!{status=child.wait()=>status.map_err(anyhow::Error::from),_=tokio::time::sleep(timeout)=>Err(anyhow::anyhow!("office.process_timeout")),_=cancel.cancelled()=>Err(anyhow::anyhow!("office.operation_cancelled"))};
    match waited {
        Ok(status)=>{tokio::time::timeout(Duration::from_secs(3),async{out.await??;err.await??;Ok::<(),anyhow::Error>(())}).await??;ensure!(status.success(),"office.process_failed: {status}");Ok(())},
        Err(error)=>{let _=child.force_terminate_tree();let _=tokio::time::timeout(Duration::from_secs(3),child.wait()).await;out.abort();err.abort();Err(error)}
    }
}
