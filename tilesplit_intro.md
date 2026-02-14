---
title: "TileSplit：保留 Ultra HDR 資訊的圖片分割工具"
date: 2026-02-13T12:00:00+08:00
draft: false
tags: [rust, ultrahdr, image-processing, jpeg, cli]
---

最近我在開發一個名為 `tilesplit` 的小工具，這是一個用 Rust 編寫的命令行應用程式，專門用來處理 Ultra HDR 格式的圖片分割。

在這篇文章中，我想分享一下為什麼需要這個工具，以及 Ultra HDR 究竟是什麼。

## 什麼是 Ultra HDR？

Ultra HDR 是 Google 在 Android 14 中引入的一種新圖片格式（基於 JPEG）。它的核心理念是**向後兼容**。

傳統的 HDR 圖片（如 10-bit HEIF 或 AVIF）在舊設備或不支援 HDR 的螢幕上顯示時，往往會出現顏色怪異或過暗的問題。Ultra HDR 聰明地解決了這個問題：

1.  **SDR 基礎圖片**：它本質上還是一個標準的 JPEG 檔案，任何支援 JPEG 的軟體都能打開它並顯示標準動態範圍（SDR）的圖像。
2.  **Gain Map（增益圖）**：它在 JPEG 的 metadata 或附加數據中嵌入了一張「增益圖」。
3.  **HDR 重建**：當在支援 HDR 的螢幕上查看時，系統會將 SDR 基礎圖片與 Gain Map 結合，計算出更亮的亮部和更豐富的細節，從而還原 HDR 效果。

這種格式讓同一張圖片既能在舊手機上正常顯示，又能在新一代 HDR 螢幕上大放異彩。

## 為什麼需要 TileSplit？

如果你只需要對普通 JPEG 圖片進行裁剪或分割（例如將一張 3D 左右格式的圖片切成兩張），市面上有無數工具可以做到（如 ImageMagick, ffmpeg 等）。

但問題在於，**大多數傳統工具在處理圖片時會丟失 Ultra HDR 的 Gain Map 資訊**。當你用這些工具裁剪一張 Ultra HDR 照片並存檔時，它通常只會保留 SDR 部分，原本絢麗的 HDR 效果就消失了。

這就是 `tilesplit` 誕生的原因。

## TileSplit 是如何工作的？

`tilesplit` 的主要目標是在分割圖片的同時，完整保留並正確處理 Ultra HDR 的數據。它的工作流程如下：

1.  **智慧偵測與解析**：
    *   程式首先讀取輸入檔案，並使用高效的 `jpegli` 庫進行初步探測。它會檢查 JPEG 的 Metadata（XMP）以及是否包含第二圖像（Gain Map）。
    *   如果 `jpegli` 無法識別，程式會回退到使用 Google 原生的 `ultrahdr` 解碼器進行更深入的解析，確保兼容各種不同設備生成的 Ultra HDR 檔案。

2.  **雙層解碼**：
    *   一旦確認為 Ultra HDR，程式會將圖片「拆解」為兩個部分：**SDR 主圖**（標準 RGB 像素數據）和 **Gain Map**（通常是單通道的亮度增益數據）。
    *   同時，它會提取關鍵的 Metadata（如 `hdrgm:GainMapMin`, `hdrgm:Gamma` 等），這些數據定義了如何將 SDR 和 Gain Map 結合成最終的 HDR 圖像。

3.  **幾何映射與分割**：
    *   根據使用者需求的比例（如 16:10 或 3:2），程式計算出 SDR 主圖的裁剪區域。
    *   **關鍵步驟**：由於 Gain Map 的解析度通常比主圖小（例如 1/4 大小），直接套用坐標會導致錯誤。`tilesplit` 會根據兩者的解析度比例，精確計算出 Gain Map 對應的裁剪區域，確保像素級別的對齊。

4.  **獨立編碼與重組**：
    *   裁剪後的 SDR 像素數據會被重新編碼為高品質的 JPEG。
    *   裁剪後的 Gain Map 數據也會被獨立編碼。
    *   最後，程式利用 `jpegli` 的高級功能，將新的 Gain Map JPEG 作為二進制數據嵌入到主圖 JPEG 中，並生成包含正確偏移量和長度的新 XMP Metadata。
    *   如果有 ICC Color Profile，也會一併保留。

這個過程確保了輸出的每一張小圖（Tile）本身都是一個完整、合法的 Ultra HDR 檔案，可以在支援的設備上獨立顯示 HDR 效果。

## 關鍵技術與相依庫

為了實現複雜的 Ultra HDR 處理，`tilesplit` 站在了幾個優秀 Rust 庫的肩膀上：

1.  **[`image`](https://crates.io/crates/image)**: Rust 生態系中最常用的圖像處理庫。我們使用它來進行基礎的圖像裁剪、格式轉換以及處理非 HDR 的標準圖片。
2.  **[`jpegli-rs`](https://crates.io/crates/jpegli-rs)**: 這是 Google `jpegli` 庫的 Rust 封裝。`jpegli` 提供了比傳統 `libjpeg` 更高的壓縮率和更多的特性。在 `tilesplit` 中，它負責處理 JPEG 的「額外數據」（Extras），讓我們能方便地存取嵌入在 JPEG 中的 XMP Metadata 和 Gain Map 二進制流。
3.  **[`ultrahdr-rs`](https://crates.io/crates/ultrahdr-rs)**: 這是對 Google 官方 `libultrahdr` 的封裝。它包含了 Ultra HDR 的核心邏輯，如 Metadata 的解析與生成、SDR 與 HDR 之間的轉換公式等。當圖片結構較為特殊時，它是我們最可靠的解析備案。

## 為什麼選擇 Rust？

開發這樣的圖像處理工具，Rust 展現了幾個顯著的優勢：

1.  **內存安全與低階控制**：處理 JPEG 結構、解析 XMP metadata 以及操作原始像素數據（Raw Pixel Data）需要精確的內存控制。Rust 的所有權模型確保了我們在操作這些緩衝區時，不會出現 Segfault 或緩衝區溢出等常見錯誤，這在處理二進制格式時至關重要。
2.  **高性能**：圖像解碼和編碼是計算密集型任務。Rust 提供了媲美 C/C++ 的執行效率，這對於需要快速處理高解析度照片的 CLI 工具來說非常重要。
3.  **優秀的 FFI（外部函數接口）支援**：Ultra HDR 的許多核心實作仍然依賴 C/C++ 庫（如 `libultrahdr` 或 `libjpeg`）。Rust 強大的 FFI 讓我們可以輕鬆地調用這些成熟的庫，同時通過封裝（Wrapper）提供安全的 Rust 介面。本專案就大量使用了對 `jpegli` 和 `ultrahdr` 的 Rust 綁定。
4.  **強大的錯誤處理**：Rust 的 `Result` 類型強制開發者處理所有可能的錯誤情況（例如檔案損壞、metadata 缺失等）。這讓 `tilesplit` 在遇到異常輸入時能給出清晰的錯誤訊息，而不是直接崩潰。

## 如何使用

`tilesplit` 是一個命令行工具。基本用法非常簡單：

```bash
# 基本用法，自動生成 output-left.jpg 和 output-right.jpg
tilesplit --input photo.jpg

# 指定輸出路徑
tilesplit --input photo.jpg --left-output left.jpg --right-output right.jpg
```

如果你對 Rust 有興趣，或者想查看源碼，可以在 GitHub 上找到這個專案（假設已開源）。

## 結語

隨著 HDR 螢幕越來越普及，像 Ultra HDR 這樣的格式將會變得越來越重要。`tilesplit` 是一個嘗試去填補現有圖像處理工具在 HDR 支援方面空白的小專案。希望這個工具對有類似需求的人有所幫助！
