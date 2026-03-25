import urllib.request
import os

url = 'https://raw.githubusercontent.com/huggingface/candle/main/candle-examples/examples/whisper/melfilters.bytes'
print("Downloading melfilters.bytes from Candle repository...")
response = urllib.request.urlopen(url)
data = response.read()

output_path = 'r2d2-cortex/src/models/melfilters_data.rs'
print(f"Generating {output_path} with {len(data)} bytes...")

with open(output_path, 'w') as f:
    f.write('pub const MEL_FILTERS_BYTES: [u8; ' + str(len(data)) + '] = [\n')
    for i in range(0, len(data), 16):
        chunk = data[i:i+16]
        f.write('    ' + ', '.join(str(b) for b in chunk) + ',\n')
    f.write('];\n')

print("Success! Air-gapped constants generated.")
