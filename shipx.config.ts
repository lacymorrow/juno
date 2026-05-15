export default {
	packageJsonPaths: [
		"package.json",
		"packages/juno-cua/package.json",
		"backend-server/package.json",
		"tauri-plugin-voice-transcription/package.json",
		"tauri-plugin-voice-transcription/api/package.json",
	],
	// cargoWorkspaces auto-detected: src-tauri/Cargo.toml
	git: {
		extraTags: ["cua-{tag}"],
		commitMessage: "release: {tag}",
		commitFlags: "",
		pushFlags: "",
	},
	npm: {
		cwd: "packages/juno-cua",
		access: "public",
	},
	steps: {
		// CI creates releases: release-tauri.yml (v*) + release-cua.yml (cua-v*)
		githubRelease: false,
		// CI handles Homebrew in release-cua.yml (needs binary SHAs, not source tarball)
		homebrew: false,
	},
};
