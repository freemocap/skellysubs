import { Box, Typography, CardContent } from "@mui/material";
import {Subtitle} from "./video-subtitle-viewer-types";
import {formatTime} from "./video-subtitle-viewer-utils";

interface CurrentSubtitleCardProps {
    currentSubtitle?: Subtitle | null;
}

export const CurrentSubtitleCard = ({ currentSubtitle }: CurrentSubtitleCardProps) => (
    <CardContent sx={{ bgcolor: "action.hover" }}>
        <Typography variant="subtitle1" gutterBottom>
            Current Subtitle
        </Typography>
        <Box sx={{ minHeight: 80, transition: "all 300ms" }}>
            {currentSubtitle ? (
                <Box sx={{ p: 2, bgcolor: "primary.light", border: "1px solid primary.main" }}>
                    <Typography variant="caption" color="text.secondary">
                        {formatTime(currentSubtitle.start)} → {formatTime(currentSubtitle.end)}
                    </Typography>
                    <Typography>{currentSubtitle.text.join(" ")}</Typography>
                </Box>
            ) : (
                <Box sx={{ p: 2, bgcolor: "background.default" }}>
                    <Typography>No subtitle at current time</Typography>
                </Box>
            )}
        </Box>
    </CardContent>
);