import { Select, MenuItem, Typography, CardContent } from "@mui/material";
import {SubtitleCue} from "./video-subtitle-viewer-types";

interface SubtitleVersionSelectorProps {
    subtitles: SubtitleCue[];
    selectedId?: string | null;
    onSelect: (id: string) => void;
}

export const SubtitleVersionSelector = ({
                                            subtitles,
                                            selectedId,
                                            onSelect
                                        }: SubtitleVersionSelectorProps) => (
    <CardContent>
        <Typography variant="subtitle1" gutterBottom>
            Subtitle Version
        </Typography>
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
    </CardContent>
);