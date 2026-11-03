# Use-case videos

The original videos remain unchanged. The `.preview.mp4` copies preserve their
full duration and audio, encoded as 1280 × 720 H.264 video with AAC audio and MP4
fast-start metadata. Each preview is smaller than 9,500,000 bytes for GitHub's
10 MB video attachment limit.

- [Game marketplace preview](ZADP_Zcash_Game.preview.mp4) · [Original](ZADP_Zcash_Game.mp4)
- [Independent discovery preview](ZADP_The_Protocol_Survives_Final.preview.mp4) · [Original](ZADP_The_Protocol_Survives_Final.mp4)

The repository README embeds GitHub-hosted attachments of these previews,
uploaded against this private repository, and retains direct links to both the
originals and preview files. Repository visibility has not been changed.

For future replacements, upload the preview through GitHub's Markdown editor
(or the attachment upload mechanism used by the official GitHub CLI) and put
the generated attachment URL on its own line. A repository MP4 link alone
is a file link, not an inline player. Keep the original-download links.

## Technical overview 02 — shared boards, independent discovery

- [Technical video](ZADP_Technical_02_Board_Sharing.mp4) — 2:32, 1280 × 720,
  H.264/AAC, 4.3 MB, English synthetic narration with burned-in subtitles.
- [English subtitles](ZADP_Technical_02_Board_Sharing.en.srt).
- [GitHub player attachment](https://github.com/user-attachments/assets/0d0a9c9a-359c-4027-b2b9-fd0723aa38d0).

The full video fits below 9.5 MB, so the README player uses the complete video.
It appears first under Architecture overview. The future sharing layer is
explicitly distinguished from existing independent reconstruction: opt-in
signed-descriptor/UIVK distribution, multiple boards with separate policies,
and local hosted visibility without spending authority.
