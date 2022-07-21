use std::time::Duration;

use serde_derive::Deserialize;

use svg2polylines::Polyline;

use crate::robot::PrintTask;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PrintMode {
    Once,
    Schedule5,
    Schedule15,
    Schedule30,
    Schedule60,
}

impl PrintMode {
    pub(crate) fn to_print_task(&self, polylines: Vec<Polyline>) -> PrintTask {
        match *self {
            PrintMode::Once => PrintTask::Once(polylines),
            PrintMode::Schedule5 => {
                PrintTask::Scheduled(Duration::from_secs(5 * 60), vec![polylines])
            }
            PrintMode::Schedule15 => {
                PrintTask::Scheduled(Duration::from_secs(15 * 60), vec![polylines])
            }
            PrintMode::Schedule30 => {
                PrintTask::Scheduled(Duration::from_secs(30 * 60), vec![polylines])
            }
            PrintMode::Schedule60 => {
                PrintTask::Scheduled(Duration::from_secs(60 * 60), vec![polylines])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_mode_to_print_task_once() {
        let mode = PrintMode::Once;
        let polylines = vec![];
        match mode.to_print_task(polylines.clone()) {
            PrintTask::Once(p) => assert_eq!(p, polylines),
            t @ _ => panic!("Task was {:?}", t),
        }
    }

    #[test]
    fn print_mode_to_print_task_every() {
        let mode = PrintMode::Schedule5;
        let polylines = vec![];
        match mode.to_print_task(polylines.clone()) {
            PrintTask::Scheduled(d, p) => {
                assert_eq!(d, Duration::from_secs(60 * 5));
                assert_eq!(p, vec![polylines]);
            }
            t @ _ => panic!("Task was {:?}", t),
        }
    }
}
