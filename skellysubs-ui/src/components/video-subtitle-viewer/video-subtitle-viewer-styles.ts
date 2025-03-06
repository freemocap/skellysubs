import { Box, styled } from "@mui/material";

export const VideoWrapper = styled(Box)({
    position: 'relative',
    backgroundColor: 'black',
    aspectRatio: '16/9'
});

export const VideoControls = styled(Box)({
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    background: 'linear-gradient(to top, rgba(0, 55,55,0.9), transparent)',
    padding: '1rem'
});
