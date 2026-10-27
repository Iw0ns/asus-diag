#![no_main]
#![no_std]
//! Diagnóstico de solo lectura para placas ASUS (UEFI). No modifica el BIOS.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::arch::x86_64::__cpuid;
use core::slice;
use uefi::prelude::*;
use uefi::table::cfg::{SMBIOS3_GUID, SMBIOS_GUID};
#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    log::info!("========================================================");
    log::info!("      ASUS Hardware Diagnostic & BIOS Advisor (UEFI)    ");
    log::info!("========================================================");
    // 1. Auditoría de CPU y Virtualización por Hardware
    let cpu = analyze_cpu();
    log::info!("Procesador: {}", cpu.brand);
    log::info!("Fabricante: {}", cpu.vendor);
    if cpu.virt_supported {
        log::info!("Virtualizacion en CPU: SOPORTADA ({})", cpu.virt_tech);
        match cpu.virt_enabled {
            Some(true) => log::info!("Virtualizacion en BIOS: HABILITADA [OK]"),
            Some(false) => {
                log::warn!("Virtualizacion en BIOS: DESHABILITADA (Bloqueada por BIOS)");
                log::info!("  Nota: Confirmalo en Windows (Admin. de tareas -> Rendimiento -> CPU).");
            }
            None => {
                log::info!("Virtualizacion en BIOS: NO CONFIRMADA (Registro no bloqueado por BIOS)");
                log::info!("  Nota: Confirmalo en Windows (Admin. de tareas -> Rendimiento -> CPU).");
            }
        }
    } else {
        log::warn!("Virtualizacion por hardware: NO DETECTADA");
    }
    // 2. Auditoría de SMBIOS (Placa Base)
    let smbios = parse_smbios();
    match &smbios.motherboard {
        Some(b) => log::info!("Placa Base: {} {}", b.manufacturer, b.product),
        None => log::warn!("Placa Base: No se pudo identificar por SMBIOS"),
    }
    if let Some(b) = &smbios.motherboard {
        if !b.manufacturer.to_ascii_uppercase().contains("ASUS") {
            log::warn!("Aviso: La placa no parece ASUS; los nombres de los menus pueden variar.");
        }
    }
    // 3. Auditoría de Módulos RAM
    log::info!("Modulos RAM instalados: {}", smbios.ram_modules.len());
    for (i, d) in smbios.ram_modules.iter().enumerate() {
        let max_str = if d.max_speed == 0 || d.max_speed == 0xFFFF {
            String::from("Desc.")
        } else {
            alloc::format!("{} MT/s", d.max_speed)
        };
        log::info!(
            " DIMM #{}: {} MB | Actual: {} MT/s | Nominal: {}",
            i + 1,
            d.size_mb,
            d.configured_speed,
            max_str
        );
    }
    // 4. Recomendaciones ASUS
    print_recommendations(&cpu, &smbios);
    log::info!("");
    log::info!("Presiona cualquier tecla para salir al gestor de arranque...");
    wait_for_keypress();
    Status::SUCCESS
}
// ------------------------------------------------------------------ CPU
struct CpuAudit {
    vendor: &'static str,
    brand: String,
    virt_supported: bool,
    virt_tech: &'static str,
    virt_enabled: Option<bool>,
}
unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags)
    );
    ((hi as u64) << 32) | (lo as u64)
}
fn analyze_cpu() -> CpuAudit {
    unsafe {
        let leaf0 = __cpuid(0);
        let max_leaf = leaf0.eax;
        let mut v = [0u8; 12];
        v[0..4].copy_from_slice(&leaf0.ebx.to_le_bytes());
        v[4..8].copy_from_slice(&leaf0.edx.to_le_bytes());
        v[8..12].copy_from_slice(&leaf0.ecx.to_le_bytes());
        let is_intel = &v == b"GenuineIntel";
        let is_amd = &v == b"AuthenticAMD";
        let vendor = if is_intel {
            "Intel"
        } else if is_amd {
            "AMD"
        } else {
            "Desconocido"
        };
        let max_ext = __cpuid(0x8000_0000).eax;
        let mut virt_supported = false;
        let mut virt_tech = "Ninguna";
        let mut virt_enabled = None;
        if is_intel && max_leaf >= 1 && (__cpuid(1).ecx & (1 << 5)) != 0 {
            virt_supported = true;
            virt_tech = "Intel VT-x";
            let fc = rdmsr(0x3A);
            let locked = (fc & 1) != 0;
            let vmx_enabled = (fc & (1 << 2)) != 0;
            virt_enabled = if locked {
                Some(vmx_enabled)
            } else {
                None
            };
        } else if is_amd && max_ext >= 0x8000_0001 && (__cpuid(0x8000_0001).ecx & (1 << 2)) != 0 {
            virt_supported = true;
            virt_tech = "AMD-V (SVM)";
            let vmcr = rdmsr(0xC001_0114);
            virt_enabled = Some((vmcr & (1 << 4)) == 0);
        }
        let mut bytes: Vec<u8> = Vec::new();
        if max_ext >= 0x8000_0004 {
            for leaf in 0x8000_0002u32..=0x8000_0004 {
                let r = __cpuid(leaf);
                bytes.extend_from_slice(&r.eax.to_le_bytes());
                bytes.extend_from_slice(&r.ebx.to_le_bytes());
                bytes.extend_from_slice(&r.ecx.to_le_bytes());
                bytes.extend_from_slice(&r.edx.to_le_bytes());
            }
        }
        let brand: String = String::from_utf8_lossy(&bytes)
            .trim_matches(|c: char| c == '\0' || c == ' ')
            .into();
        CpuAudit {
            vendor,
            brand: if brand.is_empty() { String::from("Desconocido") } else { brand },
            virt_supported,
            virt_tech,
            virt_enabled,
        }
    }
}
// --------------------------------------------------------------- SMBIOS
struct MotherboardInfo {
    manufacturer: String,
    product: String,
}
struct RamDimmInfo {
    size_mb: u32,
    configured_speed: u16,
    max_speed: u16,
}
#[derive(Default)]
struct SmbiosReport {
    motherboard: Option<MotherboardInfo>,
    ram_modules: Vec<RamDimmInfo>,
}
fn smbios_area() -> Option<&'static [u8]> {
    uefi::system::with_config_table(|tables| {
        unsafe {
            for e in tables {
                if e.guid == SMBIOS3_GUID {
                    let ep = e.address as *const u8;
                    if !ep.is_null() && slice::from_raw_parts(ep, 5) == b"_SM3_" {
                        let len = (ep.add(0x0C) as *const u32).read_unaligned() as usize;
                        let addr = (ep.add(0x10) as *const u64).read_unaligned() as usize;
                        if addr != 0 && len != 0 {
                            return Some(slice::from_raw_parts(addr as *const u8, len));
                        }
                    }
                }
            }
            for e in tables {
                if e.guid == SMBIOS_GUID {
                    let ep = e.address as *const u8;
                    if !ep.is_null() && slice::from_raw_parts(ep, 4) == b"_SM_" {
                        let len = (ep.add(0x16) as *const u16).read_unaligned() as usize;
                        let addr = (ep.add(0x18) as *const u32).read_unaligned() as usize;
                        if addr != 0 && len != 0 {
                            return Some(slice::from_raw_parts(addr as *const u8, len));
                        }
                    }
                }
            }
        }
        None
    })
}
fn u16_at(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}
fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}
fn smbios_string(strings: &[u8], index: u8) -> String {
    if index == 0 {
        return String::from("N/A");
    }
    strings
        .split(|&b| b == 0)
        .nth(index as usize - 1)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).trim().into())
        .unwrap_or_else(|| String::from("N/A"))
}
fn parse_smbios() -> SmbiosReport {
    let mut report = SmbiosReport::default();
    let Some(table) = smbios_area() else {
        return report;
    };
    let mut has_type2 = false;
    let mut pos = 0usize;
    while pos + 4 <= table.len() {
        let stype = table[pos];
        let flen = table[pos + 1] as usize;
        if stype == 127 || flen < 4 || pos + flen > table.len() {
            break;
        }
        let mut end = pos + flen;
        while end + 1 < table.len() && !(table[end] == 0 && table[end + 1] == 0) {
            end += 1;
        }
        let formatted = &table[pos..pos + flen];
        let strings = &table[pos + flen..end.min(table.len())];
        match stype {
            1 if !has_type2 && report.motherboard.is_none() && flen >= 6 => {
                report.motherboard = Some(MotherboardInfo {
                    manufacturer: smbios_string(strings, formatted[4]),
                    product: smbios_string(strings, formatted[5]),
                });
            }
            2 if !has_type2 && flen >= 6 => {
                report.motherboard = Some(MotherboardInfo {
                    manufacturer: smbios_string(strings, formatted[4]),
                    product: smbios_string(strings, formatted[5]),
                });
                has_type2 = true;
            }
            17 if flen >= 0x17 => {
                let raw = u16_at(formatted, 0x0C).unwrap_or(0);
                if raw != 0 && raw != 0xFFFF {
                    let size_mb = if raw == 0x7FFF {
                        u32_at(formatted, 0x1C).unwrap_or(0) & 0x7FFF_FFFF
                    } else if raw & 0x8000 != 0 {
                        (raw & 0x7FFF) as u32 / 1024
                    } else {
                        raw as u32
                    };
                    let max_speed = u16_at(formatted, 0x15).unwrap_or(0);
                    let configured_speed = u16_at(formatted, 0x20).unwrap_or(max_speed);
                    report.ram_modules.push(RamDimmInfo {
                        size_mb,
                        configured_speed,
                        max_speed,
                    });
                }
            }
            _ => {}
        }
        pos = end + 2;
    }
    report
}
// ------------------------------------------------------ Recomendaciones
fn print_recommendations(cpu: &CpuAudit, smbios: &SmbiosReport) {
    log::info!("");
    log::info!("============ RECOMENDACIONES (Aplicar a mano en el BIOS) ============");
    let valid_modules: Vec<&RamDimmInfo> = smbios
        .ram_modules
        .iter()
        .filter(|d| d.max_speed > 0 && d.max_speed != 0xFFFF)
        .collect();
    if valid_modules.is_empty() {
        log::info!("RAM: Velocidades nominales no reportadas por SMBIOS; estado XMP no determinado.");
    } else {
        let below = valid_modules.iter().any(|d| d.configured_speed < d.max_speed);
        if below {
            log::warn!("Tu RAM podria estar funcionando por debajo de su velocidad nominal.");
            log::info!("  Nota: El campo de velocidad de SMBIOS varia segun la placa y el firmware; tomalo como pista.");
            log::info!("  -> BIOS (F2 / Del) -> Ai Tweaker -> 'Ai Overclock Tuner'");
            if cpu.vendor == "AMD" {
                log::info!("  -> Elige: [D.O.C.P.] (en AM4) o [EXPO I] (en AM5)");
            } else {
                log::info!("  -> Elige: [XMP I]");
            }
        } else {
            log::info!("[OK] La RAM opera a la velocidad nominal reportada.");
        }
    }
    if smbios.ram_modules.len() >= 4 {
        log::info!("  Nota: Tienes 4 modulos instalados. Si XMP/EXPO resulta inestable,");
        log::info!("        utiliza el perfil I o baja un escalon la frecuencia.");
    }
    if cpu.virt_supported && cpu.virt_enabled == Some(false) {
        log::info!("\nVirtualizacion (WSL2) - Desactivada en BIOS:");
        if cpu.vendor == "AMD" {
            log::info!("  -> Advanced -> CPU Configuration -> 'SVM Mode' = [Enabled]");
        } else {
            log::info!("  -> Advanced -> CPU Configuration -> 'Intel (VMX) Virtualization Technology' = [Enabled]");
        }
    }
    log::info!("\nResizable BAR (Rendimiento GPU):");
    log::info!("  -> Advanced -> PCI Subsystem Settings -> 'Above 4G Decoding' = [Enabled]");
    log::info!("  -> luego 'Re-Size BAR Support' = [Enabled] o [Auto]");
    log::info!("\nArranque UEFI / CSM:");
    log::info!("  -> Boot -> CSM -> 'Launch CSM' = [Disabled]");
    log::warn!("  ATENCION: Si Windows se instalo en modo Legacy/MBR,");
    log::warn!("  desactivar CSM impedira que arranque.");
}
fn wait_for_keypress() {
    uefi::system::with_stdin(|stdin| {
        let _ = stdin.reset(false);
        while let Ok(Some(_)) = stdin.read_key() {}
        if let Some(ev) = stdin.wait_for_key_event() {
            let mut evs = [ev];
            let _ = uefi::boot::wait_for_event(&mut evs);
        }
        let _ = stdin.read_key();
    });
}
