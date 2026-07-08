use std::fs;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub x: f32, // normalized x coordinate (0.0 to 1.0)
    pub y: f32, // normalized y coordinate (0.0 to 1.0)
    pub w: f32, // normalized width
    pub h: f32, // normalized height
}

/// Capture a screenshot, run local OCR on it, and return recognized text blocks with normalized coordinates.
pub fn run_ocr(screenshot_bytes: &[u8]) -> anyhow::Result<Vec<OcrResult>> {
    let temp_dir = std::env::temp_dir();
    let uuid = uuid::Uuid::new_v4();
    let img_path = temp_dir.join(format!("ghost_ocr_screenshot_{}.png", uuid));
    
    // Write image bytes to a temp file
    fs::write(&img_path, screenshot_bytes)?;
    
    let result = run_platform_ocr(&img_path);
    
    // Clean up screenshot file
    let _ = fs::remove_file(&img_path);
    
    result
}

#[cfg(target_os = "macos")]
fn run_platform_ocr(img_path: &std::path::Path) -> anyhow::Result<Vec<OcrResult>> {
    let temp_dir = std::env::temp_dir();
    let uuid = uuid::Uuid::new_v4();
    let script_path = temp_dir.join(format!("ghost_ocr_script_{}.swift", uuid));
    
    let script_content = r#"
import Foundation
import Vision
import Cocoa

guard CommandLine.arguments.count > 1 else {
    print("Usage: ocr <image-path>")
    exit(1)
}
let imagePath = CommandLine.arguments[1]
guard let image = NSImage(contentsOfFile: imagePath),
      let tiffData = image.tiffRepresentation,
      let cgImage = NSImage(data: tiffData)?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    print("Error: Could not load image")
    exit(1)
}

let requestHandler = VNImageRequestHandler(cgImage: cgImage, options: [:])
let request = VNRecognizeTextRequest { request, error in
    if let error = error {
        print("Error: \(error.localizedDescription)")
        return
    }
    guard let observations = request.results as? [VNRecognizedTextObservation] else { return }
    var results: [[String: Any]] = []
    for observation in observations {
        guard let candidate = observation.topCandidates(1).first else { continue }
        let text = candidate.string
        let boundingBox = observation.boundingBox
        let result: [String: Any] = [
            "text": text,
            "x": boundingBox.origin.x,
            "y": boundingBox.origin.y,
            "w": boundingBox.size.width,
            "h": boundingBox.size.height
        ]
        results.append(result)
    }
    if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: []),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        print(jsonString)
    }
}

request.recognitionLevel = .accurate
try? requestHandler.perform([request])
"#;

    fs::write(&script_path, script_content)?;
    
    let output = Command::new("swift")
        .arg(&script_path)
        .arg(img_path)
        .output();
        
    let _ = fs::remove_file(&script_path);
    
    let output = output?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("OCR Swift script execution failed: {}", stderr);
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let results: Vec<OcrResult> = serde_json::from_str(&stdout)?;
    Ok(results)
}

#[cfg(target_os = "windows")]
fn run_platform_ocr(img_path: &std::path::Path) -> anyhow::Result<Vec<OcrResult>> {
    let temp_dir = std::env::temp_dir();
    let uuid = uuid::Uuid::new_v4();
    let script_path = temp_dir.join(format!("ghost_ocr_script_{}.ps1", uuid));
    
    let script_content = r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$imagePath = $args[0]
if (-not $imagePath) {
    Write-Error "Usage: ocr.ps1 <image-path>"
    exit 1
}
$file = Get-Item $imagePath
$stream = [Windows.Storage.Streams.FileRandomAccessStream]::OpenAsync($file.FullName, [Windows.Storage.FileAccessMode]::Read).GetAwaiter().GetResult()
$decoder = [Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream).GetAwaiter().GetResult()
$softwareBitmap = $decoder.GetSoftwareBitmapAsync().GetAwaiter().GetResult()
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
if (-not $engine) {
    $lang = New-Object Windows.Globalization.Language("en-US")
    $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($lang)
}
$result = $engine.RecognizeAsync($softwareBitmap).GetAwaiter().GetResult()
$width = $softwareBitmap.PixelWidth
$height = $softwareBitmap.PixelHeight
$out = @()
foreach ($line in $result.Lines) {
    if ($line.Words.Count -eq 0) { continue }
    $rect = $line.Words[0].BoundingRect
    $minX = $rect.X
    $minY = $rect.Y
    $maxX = $rect.X + $rect.Width
    $maxY = $rect.Y + $rect.Height
    foreach ($word in $line.Words) {
        $r = $word.BoundingRect
        if ($r.X -lt $minX) { $minX = $r.X }
        if ($r.Y -lt $minY) { $minY = $r.Y }
        if (($r.X + $r.Width) -gt $maxX) { $maxX = $r.X + $r.Width }
        if (($r.Y + $r.Height) -gt $maxY) { $maxY = $r.Y + $r.Height }
    }
    $w = $maxX - $minX
    $h = $maxY - $minY
    $normX = $minX / $width
    $normY = $minY / $height
    $normW = $w / $width
    $normH = $h / $height
    $out += @{
        text = $line.Text
        x = $normX
        y = $normY
        w = $normW
        h = $normH
    }
}
ConvertTo-Json $out
"#;

    fs::write(&script_path, script_content)?;
    
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script_path)
        .arg(img_path)
        .output();
        
    let _ = fs::remove_file(&script_path);
    
    let output = output?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("OCR PowerShell script execution failed: {}", stderr);
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let results: Vec<OcrResult> = serde_json::from_str(&stdout)?;
    Ok(results)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn run_platform_ocr(_img_path: &std::path::Path) -> anyhow::Result<Vec<OcrResult>> {
    anyhow::bail!("OCR element selectors are not supported on this platform")
}
