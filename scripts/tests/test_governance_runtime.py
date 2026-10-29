"""Check that every ai-rulez installation takes the version this checkout pins or expects."""

from pathlib import Path
import re
import unittest


class GovernanceRuntimeTests(unittest.TestCase):
    """mise.toml pins ai-rulez exactly, so the tasks run it without a runtime version check."""

    def test_should_install_latest_in_ci_and_pin_release_installations(self) -> None:
        """CI installers take the latest release; the six release-path installers use the mise.toml pin."""
        root = Path(__file__).resolve().parents[2]
        mise = (root / "mise.toml").read_text()
        assert re.search(r'(?m)^"github:Goldziher/ai-rulez" = "\d+\.\d+\.\d+"$', mise) is not None
        task_sources = [mise, *(path.read_text() for path in (root / "mise-tasks").rglob("*") if path.is_file())]
        assert len(task_sources) > 1, "mise-tasks/ holds no task files"
        for source in task_sources:
            assert "AI_RULEZ_VERSION" not in source, "a task overrides the mise.toml ai-rulez pin"
        release_paths = {"publish.yaml", "plugin-artifacts.yaml"}
        versions: dict[str, list[str]] = {"ci": [], "release": []}
        release_installs = 0
        for path in (root / ".github/workflows").glob("*"):
            source = path.read_text()
            if "xberg-io/actions/setup-ai-rulez@" in source:
                assert "source-revision:" not in source, f"{path} overrides the released AI-Rulez runtime"
            kind = "release" if path.name in release_paths else "ci"
            versions[kind].extend(re.findall(r'ai-rulez-version: "([^"\n]+)"', source))
            if kind == "release":
                release_installs += len(
                    re.findall(r"(?m)^ +install-args: [^\n]*github:Goldziher/ai-rulez(?=\s|$)", source)
                )
                assert "github:Goldziher/ai-rulez@" not in source, f"{path} overrides the mise.toml ai-rulez pin"
                assert re.search(r"MISE_\w*AI_RULEZ\w*_VERSION", source) is None, f"{path} overrides the mise.toml pin"
        assert versions["ci"] == ["latest"] * 3, versions
        assert versions["release"] == [], versions
        assert release_installs == 6, release_installs


if __name__ == "__main__":
    unittest.main()
