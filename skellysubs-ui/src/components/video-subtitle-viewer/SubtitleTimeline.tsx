import { Button, Stack } from "@mui/material";
import type {Subtitle} from "./video-subtitle-viewer-types";

interface SubtitleTimelineProps {
    subtitles: Subtitle[];
    currentSubtitle?: Subtitle | null;
    onSeek: (startTime: number) => void;
}

export const SubtitleTimeline = ({
                                     subtitles,
                                     currentSubtitle,
                                     onSeek
                                 }: SubtitleTimelineProps) => (
    <Stack direction="row" flexWrap="wrap" gap={1}>
        {subtitles.map((subtitle, index) => (
            <Button
                key={index}
                variant={currentSubtitle === subtitle ? "contained" : "outlined"}
                size="small"
                onClick={() => onSeek(subtitle.end)}>
                {index + 1}
            </Button>
        ))}
    </Stack>
);
