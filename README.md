# 🖥️ SysWatch - System Resource Monitor

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)
[![Windows](https://img.shields.io/badge/Platform-Windows-blue?style=flat-square&logo=windows)](https://github.com/yourusername/SysWatch/releases)

**SysWatch** is an elegant system resource monitor written in Rust using the egui GUI framework. Monitor processes, CPU, memory, and disk usage in real-time!

---

## ✨ Features

- 🔍 **Real-time process monitoring**
- 📊 **CPU and Memory history charts**
- 💾 **Disk information** (capacity, usage, type)
- 🎯 **Detailed process information** (PID, user, command line)
- 🔎 **Process search and filtering**
- 📁 **Export to JSON**
- ⚡ **Terminate processes**
- 🌙 **Dark theme**
- 🔋 **Energy saving mode**
- 🖱️ **Interactive column sorting**

---

## 🚀 Installation

### Option 1: Download pre-built .exe
1. Go to [Releases](https://github.com/YOUR_USERNAME/SysWatch/releases)
2. Download the latest `SysWatch.exe`
3. Run it! (no installation required)

### Option 2: Build from source
```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/SysWatch.git
cd SysWatch

# Build in release mode
cargo build --release

# Run
./target/release/SysWatch.exe
