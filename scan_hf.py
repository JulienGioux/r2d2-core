import urllib.request
repos = [
    "lmz/candle-whisper", 
    "jncraton/whisper-tiny", 
    "onnx-community/whisper-tiny", 
    "Xenova/whisper-tiny",
    "sanchit-gandhi/whisper-small",
    "distil-whisper/distil-medium.en"
]

print("Scanning HuggingFace pour mel_filters...")
for r in repos:
    # Mode Model
    url1 = f"https://huggingface.co/{r}/resolve/main/mel_filters.safetensors"
    # Mode Dataset
    url2 = f"https://huggingface.co/datasets/{r}/resolve/main/mel_filters.safetensors"
    
    for url in [url1, url2]:
        try:
            req = urllib.request.Request(url, method="HEAD")
            resp = urllib.request.urlopen(req)
            if resp.status == 200:
                print(f"✅ TROUVE: {url}")
        except Exception as e:
            pass
