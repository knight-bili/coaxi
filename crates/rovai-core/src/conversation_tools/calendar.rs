//! 时区随定义冻结；仍由Core原自动化调度循环计算与领取。
use super::*;
use chrono::{DateTime,Duration,LocalResult,NaiveDate,NaiveTime,TimeZone,Timelike,Utc};
use chrono_tz::Tz;
use crate::automation::AutomationSchedule;
use rusqlite::{Connection,OptionalExtension};

pub(crate) fn zone(value:&str)->Result<Tz>{ensure!(!value.is_empty()&&value.len()<=128,"conversation_tools.timezone_invalid");value.parse().map_err(|_|anyhow::anyhow!("conversation_tools.timezone_invalid"))}
pub(crate) fn resolve(requested:Option<&str>)->Result<(String,Option<String>)>{
    if let Some(value)=requested{let zone=zone(value)?;return Ok((zone.name().into(),None))}
    match iana_time_zone::get_timezone().ok().and_then(|name|zone(&name).ok()){
        Some(zone)=>Ok((zone.name().into(),None)),
        None=>Ok(("UTC".into(),Some("未能核对主机IANA时区，本次定义已明确保存为UTC。".into()))),
    }
}
pub(crate) fn owner_zone(connection:&Connection,id:&str)->Result<Option<String>>{
    if storage::absent(connection)?{return Ok(None)}
    connection.query_row("SELECT timezone FROM conversation_tool_automation_owner WHERE automation_id=?1",[id],|row|row.get(0)).optional().map_err(Into::into)
}
fn earlier(time:DateTime<Tz>)->DateTime<Tz>{match time.timezone().from_local_datetime(&time.naive_local()){LocalResult::Ambiguous(first,second)=>first.min(second),_=>time}}
fn expression(schedule:&AutomationSchedule)->Result<Option<String>>{
    let(at,days)=match schedule{
        AutomationSchedule::Manual|AutomationSchedule::Once{..}=>return Ok(None),
        AutomationSchedule::Cron{expression}=>return Ok(Some(expression.clone())),
        AutomationSchedule::Daily{at}=>(at,"*".into()),
        AutomationSchedule::Weekdays{at}=>(at,"1-5".into()),
        AutomationSchedule::Weekly{at,weekday}=>(at,weekday.number_from_monday().to_string()),
    };
    let time=NaiveTime::parse_from_str(at,"%H:%M").context("conversation_tools.schedule_time_invalid")?;
    Ok(Some(format!("{} {} * * {days}",time.minute(),time.hour())))
}
fn once(date:&str,at:&str,zone:Tz)->Result<Option<DateTime<Utc>>>{
    let date=NaiveDate::parse_from_str(date,"%Y-%m-%d").context("conversation_tools.schedule_date_invalid")?;let time=NaiveTime::parse_from_str(at,"%H:%M").context("conversation_tools.schedule_time_invalid")?;
    Ok(match zone.from_local_datetime(&date.and_time(time)){LocalResult::Single(value)=>Some(value.with_timezone(&Utc)),LocalResult::Ambiguous(first,second)=>Some(first.min(second).with_timezone(&Utc)),LocalResult::None=>None})
}
pub(crate) fn next(schedule:&AutomationSchedule,timezone:&str,after:DateTime<Utc>)->Result<Option<DateTime<Utc>>>{
    let zone=zone(timezone)?;
    if let AutomationSchedule::Once{date,at}=schedule{return Ok(once(date,at,zone)?.filter(|value|*value>after))}
    let Some(expression)=expression(schedule)?else{return Ok(None)};
    let cron=crate::automation::parse_cron_expression(&expression)?;let mut cursor=after.with_timezone(&zone);
    // 秋季重复区间只取较早的一次；第二次时钟回拨不会重复执行同一本地分钟。
    for _ in 0..2048{
        let candidate=cron.find_next_occurrence(&cursor,false).context("conversation_tools.cron_no_next_occurrence")?;
        let first=earlier(candidate);
        if first.with_timezone(&Utc)>after{return Ok(Some(first.with_timezone(&Utc)))}
        cursor=candidate+Duration::seconds(1);
    }
    anyhow::bail!("conversation_tools.cron_search_exhausted")
}
pub(crate) fn latest(schedule:&AutomationSchedule,timezone:&str,at:DateTime<Utc>)->Result<Option<DateTime<Utc>>>{
    let zone=zone(timezone)?;
    if let AutomationSchedule::Once{date,at:time}=schedule{return Ok(once(date,time,zone)?.filter(|value|*value<=at))}
    let Some(expression)=expression(schedule)?else{return Ok(None)};
    let cron=crate::automation::parse_cron_expression(&expression)?;
    let candidate=cron.find_previous_occurrence(&at.with_timezone(&zone),true).context("conversation_tools.cron_no_previous_occurrence")?;
    Ok(Some(earlier(candidate).with_timezone(&Utc)))
}
pub(crate) fn next_for(connection:&Connection,id:&str,schedule:&AutomationSchedule,after:DateTime<Utc>)->Result<Option<DateTime<Utc>>>{
    match owner_zone(connection,id)?{Some(timezone)=>next(schedule,&timezone,after),None=>crate::automation::next_occurrence_after(schedule,after)}
}
pub(crate) fn latest_for(connection:&Connection,id:&str,schedule:&AutomationSchedule,at:DateTime<Utc>)->Result<Option<DateTime<Utc>>>{
    match owner_zone(connection,id)?{Some(timezone)=>latest(schedule,&timezone,at),None=>crate::automation::latest_occurrence_at_or_before(schedule,at)}
}
pub(crate) fn requires_aware_edit(connection:&Connection,id:&str)->Result<bool>{
    let Some(timezone)=owner_zone(connection,id)?else{return Ok(false)};
    let action:String=connection.query_row("SELECT action FROM conversation_tool_automation_owner WHERE automation_id=?1",[id],|row|row.get(0))?;
    let local=iana_time_zone::get_timezone().ok().and_then(|name|zone(&name).ok());
    Ok(action!="launch_session"||local!=Some(zone(&timezone)?))
}
pub(crate) fn requires_enqueue_user(connection:&Connection,id:&str)->Result<bool>{
    if super::storage::absent(connection)?{return Ok(false)}
    connection.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tool_automation_owner WHERE automation_id=?1 AND action='enqueue_task')",[id],|row|row.get(0)).map_err(Into::into)
}
