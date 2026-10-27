# ⚡ ASUS UEFI Hardware Diagnostic & BIOS Advisor
<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust%202021-orange?style=for-the-badge&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Target-x86__64--unknown--uefi-blue?style=for-the-badge" alt="UEFI" />
  <img src="https://img.shields.io/badge/Safety-Read--Only%20(No%20Brick)-green?style=for-the-badge" alt="Safety" />
  <img src="https://img.shields.io/github/actions/workflow/status/Iw0ns/asus-diag/build.yml?style=for-the-badge&label=Build%20UEFI" alt="CI Status" />
</p>
Una herramienta de diagnóstico *bare-metal* para placas base **ASUS** que se ejecuta directamente desde el firmware UEFI antes de iniciar el sistema operativo. Audita el procesador, las ranuras de memoria RAM instaladas y el estado real de la virtualización, orientando al usuario con las rutas exactas de configuración en el BIOS de ASUS.
---
## 🛡️ Filosofía de Seguridad (Zero-Brick)
Muchas utilidades intentan escribir directamente en la memoria NVRAM (`Setup`) de la placa base a ciegas. En placas ASUS, un desfase de bytes en firmware de distintas versiones puede provocar que el equipo no vuelva a dar video (*brick*), requiriendo un borrado físico del CMOS.
> [!IMPORTANT]
> **Este proyecto es 100% de solo lectura.** No modifica la NVRAM ni arriesga la integridad de la placa. Realiza una inspección exhaustiva de bajo nivel e instruye al usuario de forma clara y guiada sobre qué menús activar en el setup oficial.
---
## 🔍 Características Principales
* 🧠 **Auditoría de Virtualización a Nivel Ring 0**: 
  * No se limita a consultar el silicio con `CPUID`. 
  * Lee registros MSR en tiempo real: `IA32_FEATURE_CONTROL` (MSR `0x3A`) en **Intel** y `VM_CR` (MSR `0xC0010114`) en **AMD**, confirmando si la virtualización por hardware (**VT-x** o **AMD-V/SVM**) está realmente activa en el BIOS para **WSL2** o hipervisores.
* 📋 **Parseo Robusto de SMBIOS (2.x y 3.x)**: 
  * Identificación genuina de fabricante y modelo de la placa base (Tipo 2 con respaldo a Tipo 1).
  * Soporte de memoria extendida (SMBIOS 2.7+) para módulos DDR4 y DDR5 de alta densidad.
* ⚡ **Detección de Frecuencia y Perfiles XMP/EXPO**: 
  * Inspección de cada ranura DIMM poblada (Tipo 17).
  * Compara la velocidad configurada frente a la nominal para alertar si las memorias están corriendo a velocidad base JEDEC (ej. 2133 o 4800 MT/s).
  * Exclusión de valores centinela (`0xFFFF`) para evitar falsos positivos.
* 🎮 **Asesor de Configuración ASUS**: 
  * Rutas exactas para activar **Resizable BAR** y su prerrequisito **Above 4G Decoding**.
  * Advertencias sobre el modo **CSM** (Compatibility Support Module) para evitar fallos de arranque en instalaciones MBR.
* ⌨️ **Drenado de Buffer de Teclado**: 
  * Limpia pulsaciones residuales previas (como las teclas `F8` o `Enter` del menú de arranque) evitando cierres accidentales al iniciar.
---
## 🖥️ Vista Previa en Pantalla
```text
========================================================
      ASUS Hardware Diagnostic & BIOS Advisor (UEFI)    
========================================================
Procesador: 13th Gen Intel(R) Core(TM) i7-13700K
Fabricante: Intel
Virtualizacion en CPU: SOPORTADA (Intel VT-x)
Virtualizacion en BIOS: HABILITADA [OK]
Placa Base: ASUSTeK COMPUTER INC. ROG STRIX Z790-F GAMING WIFI
Modulos RAM instalados: 2
 DIMM #1: 16384 MB | Actual: 4800 MT/s | Nominal: 6000 MT/s
 DIMM #2: 16384 MB | Actual: 4800 MT/s | Nominal: 6000 MT/s
============ RECOMENDACIONES (Aplicar a mano en el BIOS) ============
AVISO: Tu RAM podria estar funcionando por debajo de su velocidad nominal.
  Nota: El campo de velocidad de SMBIOS varia segun la placa y el firmware; tomalo como pista.
  -> BIOS (F2 / Del) -> Ai Tweaker -> 'Ai Overclock Tuner'
  -> Elige: [XMP I]
Virtualizacion (WSL2):
  -> [OK] La virtualizacion ya esta activa en el BIOS. Lista para WSL2.
Resizable BAR (Rendimiento GPU):
  -> Advanced -> PCI Subsystem Settings -> 'Above 4G Decoding' = [Enabled]
  -> luego 'Re-Size BAR Support' = [Enabled] o [Auto]
Arranque UEFI / CSM:
  -> Boot -> CSM -> 'Launch CSM' = [Disabled]
  ATENCION: Si Windows se instalo en modo Legacy/MBR, desactivar CSM impedira que arranque.
Presiona cualquier tecla para salir al gestor de arranque...
```
---
## 🚀 Cómo Usarlo
### 1. Descargar el ejecutable
Descarga el archivo `ASUS-DIAG-BOOTX64.zip` desde la sección de **Releases** o desde los artefactos de **GitHub Actions** y extráelo para obtener `BOOTX64.EFI`.
### 2. Preparar el pendrive USB
1. Formatea una memoria USB en **FAT32**.
2. Crea la siguiente estructura de carpetas en la raíz:
   ```text
   USB:\
   └── EFI\
       └── BOOT\
           └── BOOTX64.EFI
   ```
### 3. Ejecutar en tu PC
1. Conecta la memoria USB a tu equipo con placa ASUS y enciéndelo.
2. Pulsa repetidamente la tecla **`F8`** durante el arranque para abrir el menú de selección de inicio.
3. Elige la opción de tu pendrive que contenga el prefijo **`UEFI:`**.
4. Revisa las recomendaciones en pantalla y pulsa cualquier tecla para reiniciar/salir.
---
## 🛠️ Compilación desde el Código Fuente
Si prefieres compilar el binario por ti mismo:
### Requisitos locales
* [Rust](https://rustup.rs/) (Edición 2021)
* Target UEFI instalado:
  ```bash
  rustup target add x86_64-unknown-uefi
  ```
### Comando de Compilación
```bash
cargo build --release --target x86_64-unknown-uefi
```
El binario resultante se generará en:  
`target/x86_64-unknown-uefi/release/asus-diag-efi.efi` (simplemente cámbiale el nombre a `BOOTX64.EFI`).
---
## ⚖️ Licencia
