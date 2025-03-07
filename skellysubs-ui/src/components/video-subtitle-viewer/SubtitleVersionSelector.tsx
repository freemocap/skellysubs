import { Select, MenuItem, Typography, CardContent, Button, Stack } from "@mui/material";
import type {SubtitleCue} from "./video-subtitle-viewer-types";
import { Save as SaveIcon } from "@mui/icons-material"

interface SubtitleVersionSelectorProps {
    subtitles: SubtitleCue[];
    selectedId?: string | null;
    onSelect: (id: string) => void;
}

export const SubtitleVersionSelector = ({
                                            subtitles,
                                            selectedId,
                                            onSelect
                                        }: SubtitleVersionSelectorProps) => {
    const handleSave = () => {
        const selectedSub = subtitles.find(sub => sub.id === selectedId)
        if (!selectedSub) return

        const blob = new Blob([selectedSub.content], { type: "text/vtt" })
        const url = URL.createObjectURL(blob)
        const a = document.createElement("a")
        a.href = url
        a.download = `${selectedSub.name.replace(/\s+/g, "_")}_subtitles.vtt`
        document.body.appendChild(a)
        a.click()
        document.body.removeChild(a)
        URL.revokeObjectURL(url)
    }

    return (
        <CardContent>
            <Typography variant="subtitle1" gutterBottom>
                Subtitle Version
            </Typography>
            <Stack direction="row" spacing={2} alignItems="center">
                <Select
                    value={selectedId || ""}
                    onChange={(e) => onSelect(e.target.value)}
                    fullWidth
                >
                    {subtitles.map((sub) => (
                        <MenuItem key={sub.id} value={sub.id}>
                            {sub.name}
                        </MenuItem>
                    ))}
                </Select>
                <Button
                    variant="contained"
                    startIcon={<SaveIcon />}
                    onClick={handleSave}
                    disabled={!selectedId}
                >
                    Save As
                </Button>
            </Stack>
        </CardContent>
    )
}

