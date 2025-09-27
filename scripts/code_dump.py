import os


def read_files_from_directory(directory):
    file_data = []
    for root, _, files in os.walk(directory):
        for filename in files:
            filepath = os.path.join(root, filename)
            if (
                (not "pb" in filepath)
                and (not "target" in filepath)
                and (not "gen" in filepath)
                and (not "build" in filepath)
                and (not "protos" in filepath)
                and (not "icons" in filepath)
                and (not "gradle" in filepath)
                and (not "Cargo" in filepath)
                and (not ".tauri" in filepath)
                and (not ".idea" in filepath)
                and (not "proguard-rules" in filepath)
                and (not ".png" in filepath)
                and (not ".jpg" in filepath)
                and (not ".DS_Store" in filepath)
                and (not "fonts" in filepath)
            ):
                try:
                    print("open:", filepath)
                    with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
                        content = f.read()
                    relative_path = os.path.relpath(filepath, directory)
                    file_data.append((relative_path, content))
                except Exception as e:
                    print(f"Failed to read {filepath}: {e}")
    return file_data


def write_to_txt(file_data, output_path):
    with open(output_path, "w", encoding="utf-8") as f:
        for name, content in file_data:
            f.write(f"--- {name} ---\n")
            f.write(content)
            f.write("\n\n")


def main():
    tauri_src_dir = "src-tauri"
    src_dir = "src"
    output_dir = "scriptgen"
    os.makedirs(output_dir, exist_ok=True)

    data1 = read_files_from_directory(tauri_src_dir)
    data2 = read_files_from_directory(src_dir)
    data3 = read_files_from_directory("src-tauri/modules/core")

    write_to_txt(data1, os.path.join(output_dir, "code_dump_tauri.txt"))
    write_to_txt(data2, os.path.join(output_dir, "code_dump_frontend.txt"))
    write_to_txt(data3, os.path.join(output_dir, "code_dump_core.txt"))

    print(f"Output saved: {output_dir}")


if __name__ == "__main__":
    main()
