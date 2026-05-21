# Anubis OS Engine — Codex Instructions

Baca SPEC.md sepenuhnya sebelum mulai.
File ini adalah instruksi perilaku untuk Codex selama sesi ini.

## Prioritas

1. Ikuti SPEC.md — jangan improvisasi stack atau arsitektur
2. Kerjakan Step by Step sesuai urutan di Section 16
3. Setiap step harus compile sebelum lanjut ke step berikutnya
4. Jangan skip penanganan error — lihat Section 13

## Aturan kode

- Gunakan `?` operator, bukan `unwrap()` di production code
- Semua log pakai `tracing::info!()` / `tracing::warn!()` / `tracing::error!()`
- Setiap modul baru: tulis minimal satu `#[cfg(test)]` block
- Struct yang dikirim ke frontend wajib `#[derive(Serialize, Deserialize)]`
- Tauri commands selalu return `Result<T, String>`

## Jika ada ambiguitas

Cek SPEC.md Section yang relevan.
Jika tidak ada di SPEC: tanya, jangan asumsi.

## Dependency baru

DILARANG tambah crate baru tanpa konfirmasi.
Semua dep sudah final di SPEC.md Section 2 dan Section 4.
