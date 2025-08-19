import os
import subprocess
import shutil
import toml
from pathlib import Path

def find_cargo_projects(root_dir):
    """Find all directories containing Cargo.toml files"""
    cargo_projects = []
    for dirpath, _, filenames in os.walk(root_dir):
        if "Cargo.toml" in filenames:
            cargo_projects.append(Path(dirpath))
    return cargo_projects

def get_cargo_metadata(project_dir):
    """Get cargo metadata including features and target configuration"""
    metadata = {
        'name': None,
        'features': [],
        'target': None,
    }

    cargo_toml = project_dir / "Cargo.toml"

    try:
        with open(cargo_toml) as f:
            cargo_config = toml.load(f)
            metadata['name'] = cargo_config['package']['name']

            if 'package' in cargo_config:
                if 'forced-target' in cargo_config['package']:
                    metadata['target'] = cargo_config['package']['forced-target']

            if 'features' in cargo_config:
                metadata['features'] = list(cargo_config['features'].keys())
    except (FileNotFoundError, toml.TomlDecodeError) as e:
        print(f"Warning: Couldn't parse {cargo_toml}: {e}")

    return metadata

def build_project(name, project, output, target, feature=None, build_target=None):
    """Build the cargo project with optional features and target, then move the binary"""
    # Build command
    build_cmd = ["cargo", "build", "--release", "--manifest-path", str(project / "Cargo.toml")]

    if feature:
        build_cmd.extend(["--no-default-features", "--features", feature])

    build_output = output / "log" / f"{name}-{build_target or "default"}-{feature or "default"}.log"

    # Build
    try:
        print(f"Building {project} with feature: {feature or 'default'} and target: {build_target or 'default'}")
        result = subprocess.run(build_cmd, check=True, capture_output=True, text=True )
        with open(build_output, "w") as f:
            f.write("STDOUT:\n")
            f.write(result.stdout if result.stdout else "[No stdout]\n")
            f.write("\nSTDERR:\n")
            f.write(result.stderr if result.stderr else "[No stderr]\n")
    except subprocess.CalledProcessError as e:
        print(f"Failed to build {project}: {e}")
        return

    # Determine binary location based on target
    if build_target:
        target_dir = target / build_target / "release"
    else:
        target_dir = target / "release"

    if not target_dir.exists():
        print(f"No release binaries found in {target_dir}")
        return

    # Move all binaries
    dest_name = f"{name}-{build_target or "default"}-{feature or "default"}"
    dest = output / dest_name
    if dest.exists():
        print(f"Warning: {dest} already exists, overwriting")
    shutil.move(str(target_dir / name), str(dest))
    print(f"Moved {dest_name} to {output}")

def main():
    import argparse

    parser = argparse.ArgumentParser(description="Build all Cargo projects in a directory")
    parser.add_argument("--search", help="Directory to search for Cargo projects", default=".")
    parser.add_argument("--output", help="Directory to move built binaries to", default="binaries")
    parser.add_argument("--target", help="Directory to get the build results from", default=".")
    args = parser.parse_args()

    dir_search = Path(args.search)
    dir_output = Path(args.output)
    dir_target = Path(args.target)

    if not dir_search.exists():
        print(f"Error: Search directory {dir_search} does not exist")
        return

    # create output directory if it doesn't exist
    os.makedirs(dir_output / "log", exist_ok=True)

    projects = find_cargo_projects(dir_search)
    if not projects:
        print("No Cargo projects found")
        return

    print(f"Found {len(projects)} Cargo project(s)")

    for project in projects:
        metadata = get_cargo_metadata(project)

        if metadata['features']:
            # build for each feature
            for feature in metadata['features']:
                build_project(metadata['name'], project, dir_output, dir_target, feature=feature, build_target=metadata['target'])
        else:
            build_project(metadata['name'], project, dir_output, dir_target, build_target=metadata['target'])

if __name__ == "__main__":
    main()