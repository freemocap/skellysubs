class SubtitleTimeFormatter:
    """Utility class for formatting timestamps in various subtitle formats"""

    @staticmethod
    def format_time_srt(seconds: float) -> str:
        """
        Format time for SRT format: HH:MM:SS,mmm

        Args:
            seconds: Time in seconds to format

        Returns:
            Formatted time string in SRT format (00:00:00,000)
        """
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds_remainder = seconds % 60
        milliseconds = int((seconds_remainder - int(seconds_remainder)) * 1000)
        return f"{hours:02}:{minutes:02}:{int(seconds_remainder):02},{milliseconds:03}"

    @staticmethod
    def format_time_vtt(seconds: float) -> str:
        """
        Format time for VTT format: HH:MM:SS.mmm

        Args:
            seconds: Time in seconds to format

        Returns:
            Formatted time string in WebVTT format (00:00:00.000)
        """
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds_remainder = seconds % 60
        milliseconds = int((seconds_remainder - int(seconds_remainder)) * 1000)
        return f"{hours:02}:{minutes:02}:{int(seconds_remainder):02}.{milliseconds:03}"

    @staticmethod
    def format_time_ssa(seconds: float) -> str:
        """
        Format time for SSA format: H:MM:SS.hh

        Args:
            seconds: Time in seconds to format

        Returns:
            Formatted time string in SSA format (0:00:00.00)
        """
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds_remainder = seconds % 60
        centiseconds = int((seconds_remainder - int(seconds_remainder)) * 100)
        return f"{hours}:{minutes:02}:{int(seconds_remainder):02}.{centiseconds:02}"

    @staticmethod
    def format_time_markdown(seconds:float) -> str:
        """
        Format time for Markdown format using VTT format: HH:MM:SS.mmm

        Args:
            seconds: Time in seconds to format

        Returns:
            Formatted time string in Markdown format (00:00:00.000)
        """
        return SubtitleTimeFormatter.format_time_vtt(seconds)
