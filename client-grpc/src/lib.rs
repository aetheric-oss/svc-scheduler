#![doc = include_str!("../README.md")]

pub mod client;
pub mod prelude;
pub mod service;

use client::*;
use prelude::*;

use lib_common::log_macros;
use tonic::async_trait;
use tonic::transport::Channel;
