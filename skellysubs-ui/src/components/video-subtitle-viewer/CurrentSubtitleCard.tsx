import { Box, Typography, CardContent, TextField } from "@mui/material";
import type { Subtitle } from "./video-subtitle-viewer-types";
import { formatTime } from "./video-subtitle-viewer-utils";
import { useState } from "react";
import { useAppDispatch } from "../../store/hooks";
import { updateAvailableSubtitles } from "../../store/slices/available-subtitles/availableSubtitlesSlice";

interface CurrentSubtitleCardProps {
    currentSubtitle?: Subtitle | null;
    selectedSubtitleId?: string;
    subtitleContent: string;
}

export const CurrentSubtitleCard = ({
    currentSubtitle,
    selectedSubtitleId,
    subtitleContent
}: CurrentSubtitleCardProps) => {
    const [isEditing, setIsEditing] = useState(false);
    const [editText, setEditText] = useState("");
    const dispatch = useAppDispatch();

    const handleDoubleClick = () => {
        if (currentSubtitle) {
            setEditText(currentSubtitle.text.join(" "));
            setIsEditing(true);
        }
    };

    const handleEditComplete = () => {
        if (currentSubtitle && selectedSubtitleId) {
            // Find the subtitle block in the content and replace it
            const blocks = subtitleContent.split(/\n\n+/g);
            const subtitleIndex = blocks.findIndex(block => {
                const lines = block.split('\n');
                return lines.some(line =>
                    line.includes(formatTime(currentSubtitle.start)) &&
                    line.includes(formatTime(currentSubtitle.end))
                );
            });

            if (subtitleIndex !== -1) {
                const lines = blocks[subtitleIndex].split('\n');
                // Keep timing line, replace text line
                lines[lines.length - 1] = editText;
                blocks[subtitleIndex] = lines.join('\n');

                const newContent = blocks.join('\n\n');
                dispatch(updateAvailableSubtitles({
                    id: selectedSubtitleId,
                    content: newContent
                }));
            }
        }
        setIsEditing(false);
    };

    return (
        <CardContent sx={{ bgcolor: "action.hover" }}>
            <Typography variant="subtitle1" gutterBottom>
                Current Subtitle
            </Typography>
            <Box sx={{ minHeight: 80, transition: "all 300ms" }}>
                {currentSubtitle ? (
                    <Box
                        sx={{
                            p: 2,
                            bgcolor: "primary.light",
                            border: "1px solid primary.main"
                        }}
                        onDoubleClick={handleDoubleClick}
                    >
                        <Typography variant="caption" color="text.secondary">
                            {formatTime(currentSubtitle.start)} → {formatTime(currentSubtitle.end)}
                        </Typography>
                        {isEditing ? (
                            <TextField
                                fullWidth
                                multiline
                                value={editText}
                                onChange={(e) => setEditText(e.target.value)}
                                onBlur={handleEditComplete}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter' && !e.shiftKey) {
                                        e.preventDefault();
                                        handleEditComplete();
                                    }
                                }}
                                autoFocus
                            />
                        ) : (
                            <Typography>{currentSubtitle.text.join(" ")}</Typography>
                        )}
                    </Box>
                ) : (
                    <Box sx={{ p: 2, bgcolor: "background.default" }}>
                        <Typography>No subtitle at current time</Typography>
                    </Box>
                )}
            </Box>
        </CardContent>
    );
};
