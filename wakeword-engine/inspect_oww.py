import os
import glob

def main():
    home = os.path.expanduser("~")
    registry_path = os.path.join(home, ".cargo", "registry", "src", "**", "oww-rs-*", "src", "**", "*.rs")
    print(f"Searching in {registry_path}...")
    files = glob.glob(registry_path, recursive=True)
    for f in files:
        print(f"Found: {f}")
        # Read the file and look for OwwModel definition
        with open(f, "r", encoding="utf-8", errors="ignore") as file:
            content = file.read()
            if "struct OwwModel" in content or "impl OwwModel" in content:
                print(f"=== OwwModel in {f} ===")
                # Print lines around the definition
                lines = content.splitlines()
                for idx, line in enumerate(lines):
                    if "struct OwwModel" in line or "impl OwwModel" in line or "fn detection" in line:
                        start = max(0, idx - 5)
                        end = min(len(lines), idx + 40)
                        print("\n".join(f"{i+1}: {lines[i]}" for i in range(start, end)))
                        print("-" * 40)

if __name__ == "__main__":
    main()
