# Build and GitHub publishing

This source tree is a local Git repository. The currently installed GitHub connector can update an existing repo but cannot create a new one. The owner account `onecat` has no accessible `rustshot-win` repository yet. No remote was silently created.

To create a **private** GitHub repository and publish this project with the GitHub CLI, from the project directory on a Windows machine with `gh` installed and logged in:

```powershell
gh repo create onecat/rustshot-win --private --source . --remote origin --push
```

Or create an empty `rustshot-win` repo through https://github.com/new and run:

```powershell
git remote add origin https://github.com/onecat/rustshot-win.git
git push -u origin main
```

Then visit the GitHub **Actions** tab; the included Windows CI workflow will attempt to build and upload a portable ZIP. Do not trust the download until its run is green and actual Windows 11 interaction checks pass.

To restore Git history from the bundled repository artifact:

```powershell
git clone .\\RustShot-initial.git.bundle rustshot-win
cd rustshot-win
```

There are no remote dependencies or crates for this prototype. It requires the stable Windows MSVC Rust toolchain, Windows SDK and Visual Studio Build Tools.
