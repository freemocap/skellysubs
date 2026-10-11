#[cfg(desktop)]
pub(crate) mod audio;
mod browser_capture;
mod clip_capture;
mod clip_trim;
pub(crate) mod continuous;
pub(crate) mod continuous_policy;
mod live_view;
pub(crate) mod microphone;
pub(crate) mod microphone_test;
pub(crate) mod owner;
pub(crate) mod results;
mod segmentation;
pub(crate) mod transcription;
pub(crate) mod voice;
mod wav;

mod preflight;

#[cfg(all(desktop, feature = "workflow-capture"))]
mod workflow;
