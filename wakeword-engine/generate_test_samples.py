import os
import sys
import subprocess
import urllib.request

def install_and_import(package):
    try:
        __import__(package)
    except ImportError:
        print(f"Installing {package}...")
        subprocess.check_call([sys.executable, "-m", "pip", "install", package])

def convert_mp3_to_wav(mp3_path, wav_path):
    # Convert MP3 to 16kHz mono 16-bit PCM WAV using ffmpeg
    subprocess.run([
        "ffmpeg", "-y", "-i", mp3_path,
        "-acodec", "pcm_s16le",
        "-ac", "1",
        "-ar", "16000",
        wav_path
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def generate_tts_wav(text, lang, tld, slow, wav_path, temp_mp3_path):
    from gtts import gTTS
    tts = gTTS(text=text, lang=lang, tld=tld, slow=slow)
    tts.save(temp_mp3_path)
    convert_mp3_to_wav(temp_mp3_path, wav_path)
    if os.path.exists(temp_mp3_path):
        os.remove(temp_mp3_path)

def main():
    samples_dir = os.path.join("tests", "samples")
    if not os.path.exists(samples_dir):
        os.makedirs(samples_dir)
        print(f"Created directory: {samples_dir}")

    # Ensure gTTS is installed
    install_and_import("gTTS")

    # 1. Download negative sample (human voice saying english sentences without 'Alexa')
    neg_url = "https://raw.githubusercontent.com/Uberi/speech_recognition/master/examples/english.wav"
    neg_path = os.path.join(samples_dir, "english_negative.wav")
    if not os.path.exists(neg_path):
        print(f"Downloading negative sample from {neg_url}...")
        try:
            urllib.request.urlretrieve(neg_url, neg_path)
            print(f"Downloaded negative sample to {neg_path}")
        except Exception as e:
            print(f"Failed to download negative sample: {e}")
    else:
        print("Negative sample english_negative.wav already exists.")

    temp_mp3 = os.path.join(samples_dir, "temp.mp3")

    # Define positive variations to generate
    positives = [
        ("Alexa", "en", "com", False, "alexa_positive.wav"),      # Default / US Accent
        ("Alexa", "en", "co.uk", False, "alexa_uk.wav"),         # UK Accent
        ("Alexa", "en", "com.au", False, "alexa_au.wav"),        # AU Accent
        ("Alexa", "en", "com", True, "alexa_slow.wav"),          # Slow pace
        ("Alexa, turn on the lights", "en", "com", False, "alexa_command.wav") # Command phrase
    ]

    print("\nGenerating positive 'Alexa' wake word samples...")
    for text, lang, tld, slow, filename in positives:
        path = os.path.join(samples_dir, filename)
        print(f"  -> Generating '{text}' (lang={lang}, tld={tld}, slow={slow}) to {filename}...")
        try:
            generate_tts_wav(text, lang, tld, slow, path, temp_mp3)
        except Exception as e:
            print(f"     Failed to generate {filename}: {e}")

    # Define negative variations to generate
    negatives = [
        ("Alex", "en", "com", False, "near_miss_alex.wav"),
        ("Alexis", "en", "com", False, "near_miss_alexis.wav"),
        ("Alexander", "en", "com", False, "near_miss_alexander.wav"),
        ("Computer, what time is it?", "en", "com", False, "negative_computer.wav"),
        ("Hey Google", "en", "com", False, "negative_google.wav"),
        ("Hey Siri", "en", "com", False, "negative_siri.wav")
    ]

    print("\nGenerating negative / non-wake word samples...")
    for text, lang, tld, slow, filename in negatives:
        path = os.path.join(samples_dir, filename)
        print(f"  -> Generating '{text}' (lang={lang}, tld={tld}, slow={slow}) to {filename}...")
        try:
            generate_tts_wav(text, lang, tld, slow, path, temp_mp3)
        except Exception as e:
            print(f"     Failed to generate {filename}: {e}")

    print("\nAll sample generation complete!")

if __name__ == "__main__":
    main()
