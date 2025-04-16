import React from "react";
import "./GooeyLoader.css"; // We'll create this CSS file next

const GooeyLoader: React.FC = () => {
  return (
    <div className="gooey-loader-container">
      {/* SVG for the filter definition */}
      <svg
        xmlns="http://www.w3.org/2000/svg"
        version="1.1"
        style={{ display: "none" }}
      >
        <defs>
          <filter id="gooey">
            {/* Blur the elements */}
            <feGaussianBlur
              in="SourceGraphic"
              stdDeviation="10"
              result="blur"
            />
            {/* Enhance contrast to create sharp edges */}
            <feColorMatrix
              in="blur"
              mode="matrix"
              values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 18 -7"
              result="gooey"
            />
            {/* Optional: Composite original back if needed, often omitted for pure gooey effect */}
            {/* <feComposite in="SourceGraphic" in2="gooey" operator="atop" /> */}
          </filter>
        </defs>
      </svg>

      {/* The visible loader elements that the filter applies to */}
      <div className="loader">
        {/* Multiple dots for the effect */}
        <div className="dot"></div>
        <div className="dot"></div>
        <div className="dot"></div>
        <div className="dot"></div>
        <div className="dot"></div>
      </div>
    </div>
  );
};

export default GooeyLoader;
