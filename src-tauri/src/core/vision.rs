//! Visual regression testing and image processing.
//! Uses SSIM (Structural Similarity Index) for comparing screenshots.

use image::{DynamicImage, GenericImageView, ImageBuffer, Luma};
use std::path::Path;

/// Calculate SSIM (Structural Similarity Index) between two images
pub fn calculate_ssim(img1: &DynamicImage, img2: &DynamicImage) -> f32 {
    let (w, h) = img1.dimensions();
    
    if img2.dimensions() != (w, h) {
        return 0.0;
    }

    // Convert to grayscale
    let gray1 = img1.to_luma();
    let gray2 = img2.to_luma();

    // Simple SSIM approximation
    let data1: Vec<f32> = gray1.pixels().map(|p| p[0] as f32 / 255.0).collect();
    let data2: Vec<f32> = gray2.pixels().map(|p| p[0] as f32 / 255.0).collect();

    // Calculate mean
    let mean1: f32 = data1.iter().sum::<f32>() / data1.len() as f32;
    let mean2: f32 = data2.iter().sum::<f32>() / data2.len() as f32;

    // Calculate variance
    let var1: f32 = data1.iter().map(|x| (x - mean1).powi(2)).sum::<f32>() / data1.len() as f32;
    let var2: f32 = data2.iter().map(|x| (x - mean2).powi(2)).sum::<f32>() / data2.len() as f32;

    // Calculate covariance
    let cov: f32 = data1.iter()
        .zip(data2.iter())
        .map(|(x, y)| (x - mean1) * (y - mean2))
        .sum::<f32>() / data1.len() as f32;

    // SSIM constants
    let c1 = 0.01_f32.powi(2);
    let c2 = 0.03_f32.powi(2);

    // SSIM formula
    let numerator = (2.0 * mean1 * mean2 + c1) * (2.0 * cov + c2);
    let denominator = (mean1.powi(2) + mean2.powi(2) + c1) * (var1 + var2 + c2);

    if denominator == 0.0 {
        1.0
    } else {
        numerator / denominator
    }
}

/// Capture a screenshot of the entire screen
/// Returns the image as RGB bytes
pub fn capture_screenshot() -> anyhow::Result<Vec<u8>> {
    // Platform-specific screenshot capture
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("screencapture")
            .args(&["-x", "-t", "png", "-"])
            .output()?;
        
        if output.status.success() {
            Ok(output.stdout)
        } else {
            anyhow::Err(anyhow::anyhow!("screencapture failed"))
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows implementation would use BitBlt or similar
        // For now, return empty (placeholder)
        anyhow::Err(anyhow::anyhow!("Windows screenshot not implemented"))
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::Err(anyhow::anyhow!("Unsupported platform for screenshot"))
    }
}

/// Load an image from disk
pub fn load_image(path: &str) -> anyhow::Result<DynamicImage> {
    let img = image::open(Path::new(path))?;
    Ok(img)
}

/// Save an image to disk
pub fn save_image(img: &DynamicImage, path: &str) -> anyhow::Result<()> {
    img.save(Path::new(path))?;
    Ok(())
}

/// Compare two images and return similarity score
pub fn compare_images(baseline_path: &str, current: &DynamicImage) -> anyhow::Result<f32> {
    let baseline = load_image(baseline_path)?;
    let similarity = calculate_ssim(&baseline, current);
    Ok(similarity)
}

/// Create a thumbnail of an image
pub fn create_thumbnail(img: &DynamicImage, max_size: u32) -> DynamicImage {
    img.thumbnail(max_size, max_size)
}

/// Convert image to base64 for transmission
pub fn image_to_base64(img: &DynamicImage) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    base64::encode(&bytes)
}

/// Decode base64 to image
pub fn base64_to_image(data: &str) -> anyhow::Result<DynamicImage> {
    let bytes = base64::decode(data)?;
    let img = image::load_from_memory(&bytes)?;
    Ok(img)
}