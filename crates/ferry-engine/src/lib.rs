//! 运行时组装：把 core、platform、net、store、ipc 组合成可运行的后台引擎。
//!
//! 只依赖 ferry-platform 的 trait，具体平台实现由应用注入。层级：L3。

#![forbid(unsafe_code)]
