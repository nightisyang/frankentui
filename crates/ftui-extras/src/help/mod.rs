#![forbid(unsafe_code)]

//! Contextual help system with tooltips and guided tours.
//!
//! This module provides tooltip widgets that integrate with the focus
//! system to display contextual help near focused widgets.

mod tooltip;

pub use tooltip::{Tooltip, TooltipConfig, TooltipPosition, TooltipState};
